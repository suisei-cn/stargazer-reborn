use crate::rpc::ResponseObject;

/// Represent request invocation. For more information, see [module doc](index.html#request).
pub trait Request {
    const METHOD: &'static str;
    type Res: Response;
}

/// Represent returned response data. For more information, see [module doc](index.html#response1).
pub trait Response: Sized {
    fn is_successful(&self) -> bool;

    fn packed(self) -> ResponseObject<Self> {
        let success = self.is_successful();
        ResponseObject::new(self, success)
    }
}
