use tokio::sync::broadcast;

pub struct MsgHub {
    tx: broadcast::Sender<(String, String)>,
    participants: Vec<String>,
    auto_broadcast: bool,
}

impl MsgHub {
    pub fn new(participants: Vec<String>, auto_broadcast: bool) -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            tx,
            participants,
            auto_broadcast,
        }
    }

    pub fn sender(&self) -> broadcast::Sender<(String, String)> {
        self.tx.clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<(String, String)> {
        self.tx.subscribe()
    }

    pub fn participants(&self) -> &[String] {
        &self.participants
    }

    pub fn is_auto_broadcast(&self) -> bool {
        self.auto_broadcast
    }

    pub fn set_auto_broadcast(&mut self, enabled: bool) {
        self.auto_broadcast = enabled;
    }
}