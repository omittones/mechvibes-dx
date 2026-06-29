use std::time::Duration;

pub struct Delay;

impl Delay {
    pub async fn key_event() {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    pub async fn ms(ms: u64) {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }
}
