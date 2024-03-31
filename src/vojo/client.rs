#[derive(Clone)]
pub struct Client {
    pub dbindex: usize,
    pub auth: bool,
    pub data: Vec<u8>,
}
impl Client {
    pub fn new() -> Self {
        Client {
            dbindex: 0,
            auth: false,
            data: Vec::new(),
        }
    }
}
