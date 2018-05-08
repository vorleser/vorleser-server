use helpers::db::init_test_db_pool;
use helpers;
use diesel::prelude::*;
use models::user::User;
use rocket::local::{Client, LocalResponse};
use rocket::Response;
use rocket::http::{Status, Method, Header, ContentType};
use serde_json::{self, Value};
use worker::scanner::Scanner;
use models::library::Library;
use regex::Regex;
use config;

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
        let pool = init_test_db_pool();
        let user = User::create(&"test@test.com", &"lol", &*pool.get().unwrap())
            .expect("Error saving user");
        println!("Before each {:?}", pool.state());

        let rocket = helpers::rocket::factory(pool.clone(), config::load_config_from_path(&"test-data/test-config.toml").unwrap()).unwrap();
        let client = Client::new(rocket).unwrap();

        let login_data = json!({"email": "test@test.com", "password": "lol"});
        let mut auth_response = post(&client, "/api/auth/login", &login_data, None);
        let auth_data: Value = serde_json::from_str(&auth_response.body_string().expect("no body string")).expect("JSON failed");
        let auth_token = &auth_data.get("secret").expect("no auth token").as_str().expect("not valid utf8");
    }

    it "should let you login" {
        let data = json!({"email": "test@test.com", "password": "lol"});
        println!("{}", data.to_string());
        let mut res = post(&client, "/api/auth/login", &data, None);
        assert_eq!(res.status(), Status::Ok);

        let data: Value = serde_json::from_str(&res.body_string().expect("no body string")).expect("JSON failed");
        let secret = &data.get("secret").expect("no auth token").as_str().expect("not valid utf8");
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
        let res2 = get(&client, "/api/auth/whoami", Some("de362999-55a1-4d91-9adc-b2ca2c013c97"));
        assert_eq!(res2.status(), Status::Unauthorized);
    }

    it "should log you out" {
        let second_auth_token = {
            let data = json!({"email": "test@test.com", "password": "lol"});
            let mut login_resp = post(&client, "/api/auth/login", &data, None);

            let data: Value = serde_json::from_str(
                &login_resp.body_string()
                    .expect("no body string")
            ).expect("JSON failed");
            &data.get("secret")
                .expect("no auth token")
                .as_str()
                .expect("not valid utf8")
                .to_owned()
        };

        let logout_resp = post(
            &client, "/api/auth/logout", &serde_json::Value::Null, Some(auth_token)
        );
        assert_eq!(logout_resp.status(), Status::Ok);
        let whoami_resp = get(&client, "/api/auth/whoami", Some(auth_token));
        assert_eq!(whoami_resp.status(), Status::Unauthorized);

        let whoami_resp_second = get(&client, "/api/auth/whoami", Some(second_auth_token));
        assert_eq!(whoami_resp_second.status(), Status::Ok);
    }

    it "should logout everyone" {
        let second_auth_token = {
            let data = json!({"email": "test@test.com", "password": "lol"});
            let mut login_resp = post(&client, "/api/auth/login", &data, None);

            let data: Value = serde_json::from_str(
                &login_resp.body_string()
                    .expect("no body string")
            ).expect("JSON failed");
            &data.get("secret")
                .expect("no auth token")
                .as_str()
                .expect("not valid utf8")
                .to_owned()
        };

        let logout_resp = post(
            &client, "/api/auth/logout_all", &serde_json::Value::Null, Some(auth_token)
        );

        assert_eq!(logout_resp.status(), Status::Ok);
        let whoami_resp = get(&client, "/api/auth/whoami", Some(auth_token));
        assert_eq!(whoami_resp.status(), Status::Unauthorized);
        let whoami_resp = get(&client, "/api/auth/whoami", Some(second_auth_token));
        assert_eq!(whoami_resp.status(), Status::Unauthorized);
    }

    it "should show libraries" {
        let res = get(&client, "/api/libraries", Some(auth_token));
        assert_eq!(res.status(), Status::Ok);
    }

    describe! libraries {
        before_each {
            let path = "data";
            let regex = "^[^/]+$";
            let mut library = Library::create(path.to_owned(), regex.to_owned(), &*pool.get().unwrap()).unwrap();
            let mut scanner = Scanner {
                regex: Regex::new(regex).unwrap(),
                library: library,
                pool: pool.clone(),
                config: config::load_config_from_path(&"test-data/test-config.toml").unwrap()
            };
            scanner.incremental_scan();
        }

        it "get can some books" {
            let mut res = get(&client, "/api/libraries", Some(auth_token));
            println!("Libraries: {:?}", res.body_string());
            assert_eq!(res.status(), Status::Ok);
        }
    }

}
