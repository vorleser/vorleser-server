use helpers::db::init_db_pool;
use helpers;
use diesel::prelude::*;
use models::user::UserModel;
use rocket::local::{Client, LocalResponse};
use rocket::Response;
use rocket::http::{Status, Method, Header, ContentType};
use serde_json::{self, Value};

fn post<'a>(client: &'a Client, url: &'a str, data: &Value, auth: Option<&str>) -> LocalResponse<'a> {
    if let Some(token) = auth {
        client.post(url)
            .header(Header::new("Authorization", token.to_owned()))
            .header(ContentType::JSON)
            .body(data.to_string())
            .dispatch()
    } else {
        client.post(url)
            .header(ContentType::JSON)
            .body(data.to_string())
            .dispatch()
    }
}

fn get<'a>(client: &'a Client, url: &'a str, auth: Option<&str>) -> LocalResponse<'a> {
    if let Some(token) = auth {
        client.get(url)
            .header(Header::new("Authorization", token.to_owned()))
            .dispatch()
    } else {
        client.get(url)
            .dispatch()
    }
}

describe! api_tests {
    before_each {
        let mut pool = init_db_pool();
        let conn = &*pool.get().unwrap();
        conn.execute("TRUNCATE audiobooks, chapters, playstates, users RESTART IDENTITY CASCADE").unwrap();
        let rocket = helpers::rocket::factory(pool);
        let client = Client::new(rocket).unwrap();
        let user = UserModel::create(&"test@test.com", &"lol", conn).expect("Error saving user");
        let login_data = json!({"email": "test@test.com", "password": "lol"});
        let mut auth_response = post(&client, "/api/auth/login", &login_data, None);
        let auth_data: Value = serde_json::from_str(&auth_response.body_string().expect("no body string")).expect("JSON failed");
        let auth_token = &auth_data.get("id").expect("no auth token").as_str().expect("not valid utf8");
    }

    it "should let you login" {
        let data = json!({"email": "test@test.com", "password": "lol"});
        println!("{}", data.to_string());
        let mut res = post(&client, "/api/auth/login", &data, None);
        assert_eq!(res.status(), Status::Ok);

        let data: Value = serde_json::from_str(&res.body_string().expect("no body string")).expect("JSON failed");
        let secret = &data.get("id").expect("no auth token").as_str().expect("not valid utf8");
        let res2 = get(&client, "/api/auth/whoami", Some(secret));
        assert_eq!(res2.status(), Status::Ok);
    }

    it "should not let you in with the wrong username or password" {
        let data = json!({"email": "test@test.com", "password": "lola"});
        let mut res = post(&client, "/api/auth/login", &data, None);
        assert_eq!(res.status(), Status::Unauthorized);
        let data = json!({"email": "test@testa.com", "password": "lol"});
        let mut res2 = post(&client, "/api/auth/login", &data, None);
        assert_eq!(res2.status(), Status::Unauthorized);
    }

    it "should not work with a wrong auth token" {
        let res = get(&client, "/api/auth/whoami", Some("secret"));
        assert_eq!(res.status(), Status::BadRequest);
        // TODO: test with valid uuid, result should then be unauthorized
    }

    it "should show libraries" {
        let res = get(&client, "/api/libraries", Some(auth_token));
        assert_eq!(res.status(), Status::Ok);
    }
}
