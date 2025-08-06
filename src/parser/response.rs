/// A command response to send to a client
#[derive(PartialEq, Debug)]
pub enum Response {
    /// No data
    Nil,
    /// A number
    Integer(i64),
    /// Binary data
    Data(Vec<u8>),
    /// A simple error string
    Error(String),
    /// A simple status string
    Status(String),
    /// An array of responses that may mix different types
    Array(Vec<Response>),
}

impl Response {
    /// Serializes the response into an array of bytes using Redis protocol.
    pub fn as_bytes(&self) -> Vec<u8> {
        match self {
            Response::Nil => b"$-1\r\n".to_vec(),
            Response::Data(d) => [
                &b"$"[..],
                &d.len().to_string().into_bytes()[..],
                b"\r\n",
                &d[..],
                b"\r\n",
            ]
            .concat(),
            Response::Integer(i) => [&b":"[..], &i.to_string().into_bytes()[..], b"\r\n"].concat(),
            Response::Error(d) => [&b"-"[..], (*d).as_bytes(), b"\r\n"].concat(),
            Response::Status(d) => [
                &b"+"[..],
                (*d).as_bytes(),
                &"\r\n".to_owned().into_bytes()[..],
            ]
            .concat(),
            Response::Array(a) => [
                &b"*"[..],
                &a.len().to_string().into_bytes()[..],
                b"\r\n",
                &(a.iter().map(|el| el.as_bytes()).collect::<Vec<_>>()[..].concat())[..],
            ]
            .concat(),
        }
    }

    /// Returns true if and only if the response is an error.
    pub fn is_error(&self) -> bool {
        matches!(*self, Response::Error(_))
    }

    /// Is the response a status
    pub fn is_status(&self) -> bool {
        matches!(*self, Response::Status(_))
    }
}
