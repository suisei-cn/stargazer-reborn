use serde::{de::DeserializeOwned, Serialize};

use crate::rpc::{RequestObject, ResponseObject};

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

pub trait Response: Serialize + Sized {
    fn is_successful(&self) -> bool;

    fn packed(self) -> ResponseObject<Self> {
        let success = self.is_successful();
        ResponseObject::new(self, success)
    }
}
