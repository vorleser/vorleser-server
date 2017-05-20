use helpers::db::init_db_pool;
use helpers;
use diesel::prelude::*;
use models::user::UserModel;
use rocket::testing::MockRequest;
use rocket::http::{Status, Method};
use rocket::http::ContentType;

describe! api_tests {
    before_each {
        let mut pool = init_db_pool();
        let conn = &*pool.get().unwrap();
        let rocket = helpers::rocket::factory(pool);
        let user = UserModel::create(&"test@test.test", &"lol", conn).expect("Error saving user");
    }

    after_each {
        conn.execute("TRUNCATE audiobooks, chapters, playstates, users RESTART IDENTITY CASCADE").unwrap();
    }

    it "should let you login" {
        let data = json!({"email": "test@test.test", "password": "lol"});
        let mut req = MockRequest::new(Method::Post, "/api/auth/login")
                .header(ContentType::JSON)
                .body(data.to_string());
        let mut res = req.dispatch_with(&rocket);
        assert_eq!(res.status(), Status::Ok);
    }
}
