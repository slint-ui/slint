use servo::EventLoopWaker;
use smol::channel::Sender;

#[derive(Clone)]
pub struct Waker(Sender<()>);

impl Waker {
    pub fn new(sender: Sender<()>) -> Self {
        Self(sender)
    }
}

impl EventLoopWaker for Waker {
    fn wake(&self) {
        self.0.try_send(()).expect("Failed to wake event loop");
    }

    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(Self(self.0.clone()))
    }
}
