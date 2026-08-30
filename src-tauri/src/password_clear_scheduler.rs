use std::{
    io,
    sync::mpsc::{self, Receiver, RecvTimeoutError, SendError, Sender},
    thread,
    time::{Duration, Instant},
};

use vault_platform_windows::clipboard::PasswordClipboardReceipt;

const PASSWORD_CLEAR_DELAY: Duration = Duration::from_secs(30);

/// 用一个常驻线程管理密码清理截止时间，避免每次复制都创建休眠线程。
pub(crate) struct PasswordClearScheduler {
    sender: Sender<PasswordClipboardReceipt>,
}

impl PasswordClearScheduler {
    pub(crate) fn new() -> io::Result<Self> {
        let (sender, receiver) = mpsc::channel::<PasswordClipboardReceipt>();
        thread::Builder::new()
            .name("password-clipboard-clear".to_owned())
            .spawn(move || {
                run_scheduler(receiver, PASSWORD_CLEAR_DELAY, |receipt| {
                    // 凭证会再次校验剪贴板所有权；用户后来复制的内容不会被清除。
                    let _ = receipt.clear_if_owned();
                });
            })?;
        Ok(Self { sender })
    }

    /// 发送失败时把凭证还给 command，由调用方立即执行安全清理。
    pub(crate) fn schedule(
        &self,
        receipt: PasswordClipboardReceipt,
    ) -> Result<(), PasswordClipboardReceipt> {
        self.sender
            .send(receipt)
            .map_err(|SendError(receipt)| receipt)
    }
}

/// 接收端只保存最新任务；新任务到来会重新开始完整等待周期。
fn run_scheduler<T>(receiver: Receiver<T>, delay: Duration, mut clear: impl FnMut(T)) {
    let mut pending: Option<(T, Instant)> = None;

    loop {
        match pending.take() {
            None => match receiver.recv() {
                Ok(task) => pending = Some((task, Instant::now() + delay)),
                Err(_) => break,
            },
            Some((task, deadline)) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                match receiver.recv_timeout(remaining) {
                    Ok(replacement) => {
                        // 丢弃旧凭证是安全的：新复制已经改变剪贴板 sequence。
                        pending = Some((replacement, Instant::now() + delay));
                    }
                    Err(RecvTimeoutError::Timeout) => clear(task),
                    Err(RecvTimeoutError::Disconnected) => {
                        // 应用关闭或调度器被销毁时，不再等待剩余时间。
                        clear(task);
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::run_scheduler;
    use std::{sync::mpsc, thread, time::Duration};

    #[test]
    fn password_clear_scheduler_keeps_only_latest_rapid_task() {
        let (task_sender, task_receiver) = mpsc::channel();
        let (cleared_sender, cleared_receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            run_scheduler(task_receiver, Duration::from_secs(1), |task| {
                cleared_sender.send(task).unwrap();
            });
        });

        for task in 0..100 {
            task_sender.send(task).unwrap();
        }
        drop(task_sender);
        worker.join().unwrap();

        assert_eq!(cleared_receiver.into_iter().collect::<Vec<_>>(), vec![99]);
    }

    #[test]
    fn password_clear_scheduler_resets_deadline_for_replacement() {
        let (task_sender, task_receiver) = mpsc::channel();
        let (cleared_sender, cleared_receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            run_scheduler(task_receiver, Duration::from_millis(200), |task| {
                cleared_sender.send(task).unwrap();
            });
        });

        task_sender.send("first").unwrap();
        thread::sleep(Duration::from_millis(100));
        task_sender.send("latest").unwrap();

        // 从首次任务算已超过 200ms，但从替换任务算还未到期。
        assert!(
            cleared_receiver
                .recv_timeout(Duration::from_millis(130))
                .is_err()
        );
        assert_eq!(
            cleared_receiver
                .recv_timeout(Duration::from_millis(150))
                .unwrap(),
            "latest"
        );

        drop(task_sender);
        worker.join().unwrap();
    }

    #[test]
    fn password_clear_scheduler_clears_pending_task_when_disconnected() {
        let (task_sender, task_receiver) = mpsc::channel();
        let (cleared_sender, cleared_receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            run_scheduler(task_receiver, Duration::from_secs(60), |task| {
                cleared_sender.send(task).unwrap();
            });
        });

        task_sender.send(7).unwrap();
        drop(task_sender);
        worker.join().unwrap();

        assert_eq!(cleared_receiver.try_recv().unwrap(), 7);
    }
}
