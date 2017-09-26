use uuid::Uuid;
use models::user::ApiToken;

#[derive(Serialize, Deserialize, Debug)]
pub struct TokenSerializer {
    pub secret: Uuid
}


impl From<ApiToken> for TokenSerializer {
    fn from(model: ApiToken) -> Self {
        TokenSerializer {
            secret: model.id
        }
    }
}
