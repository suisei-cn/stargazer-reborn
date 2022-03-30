use serde::{de::DeserializeOwned, Serialize};

use crate::rpc::{RequestObject, ResponseObject};

/// Represent request invocation. For more information, see [module doc](index.html#request).
pub trait Request: DeserializeOwned {
    const METHOD: &'static str;
    type Res: Response;

    fn packed(self) -> RequestObject<Self> {
        RequestObject {
            method: Self::METHOD.to_owned(),
            params: self,
        }
    }
}

/// Represent returned response data. For more information, see [module doc](index.html#response1).
pub trait Response: Serialize + Sized {
    fn is_successful(&self) -> bool;

    fn packed(self) -> ResponseObject<Self> {
        let success = self.is_successful();
        ResponseObject::new(self, success)
    }
}
