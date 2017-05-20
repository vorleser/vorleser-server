use helpers::db::init_db_pool;
use helpers;
use diesel::prelude::*;
use models::user::UserModel;
use rocket::testing::MockRequest;
use rocket::http::{Status, Method, Header, ContentType};
use serde_json::{self, Value};

describe! api_tests {
    before_each {
        let mut pool = init_db_pool();
        let conn = &*pool.get().unwrap();
        conn.execute("TRUNCATE audiobooks, chapters, playstates, users RESTART IDENTITY CASCADE").unwrap();
        let rocket = helpers::rocket::factory(pool);
        let user = UserModel::create(&"test@test.test", &"lol", conn).expect("Error saving user");
    }

    it "should let you login" {
        let data = json!({"email": "test@test.test", "password": "lol"});
        let mut req = MockRequest::new(Method::Post, "/api/auth/login")
                .header(ContentType::JSON)
                .body(data.to_string());
        let mut res = req.dispatch_with(&rocket);
        assert_eq!(res.status(), Status::Ok);

        let data: Value = serde_json::from_str(&res.body().unwrap().into_string().unwrap()).unwrap();
        let secret = &data.get("id").unwrap().as_str().unwrap();
        let mut req2 = MockRequest::new(Method::Get, "/api/auth/whoami")
                .header(Header::new("Authorization", secret.to_string()));
        let mut whoami_res = req2.dispatch_with(&rocket);
        assert_eq!(whoami_res.status(), Status::Ok);
    }
}
