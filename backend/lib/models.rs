use std::fmt;
use std::fmt::Formatter;
use base64::DecodeError;
use diesel_migrations::name;
use moon::{chrono, Duration, Utc};
use rand::{Error, Rng, RngCore};
use rand::distributions::Distribution;
use crate::schema::Users;
use crate::schema::Sessions;
use serde::Deserialize;

#[derive(Queryable, Identifiable)]
#[table_name = "Users"]
pub struct User {
    id: u64,
    pub username: String,
    hash: String,
}
impl User {
    pub fn verify_with_password(&self, password: &str) -> Result<bool, argon2::Error> {
        argon2::verify_encoded(self.hash.as_str(), password.as_bytes())
    }
}

/*NOTE: The definition of a session or a session token may change in the future.
    However this should not affect any api calls. To the user, a token may always be interpreted
    as an opaque key.
 */
#[derive(Queryable, Identifiable, PartialEq, Associations)]
#[belongs_to(User, foreign_key = "user_id")]
#[table_name="Sessions"]
pub struct Session {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub token: String,
    expires: chrono::NaiveDateTime
}

impl Session {
    pub fn is_valid(&self) -> bool {
        self.expires >= Utc::now().naive_utc()
    }
}
#[derive(Insertable)]
#[table_name="Sessions"]
pub struct NewSession {
    user_id: u64,
    pub(crate) token: String,
    expires: chrono::NaiveDateTime
}

impl NewSession {
    pub fn new(user: &User) -> Self {
        //TODO: Read size from some config file maybe
        let mut rng = rand::thread_rng();
        //TODO: This doesn't adhere to oauth2 std
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                            abcdefghijklmnopqrstuvwxyz\
                            0123456789)(*&^%$#@!~";
        let token = (0..64)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect();

        //TODO: This is stupid. In the future we should use timestamps or SQL specific stuff
        //TODO: Read this stuff from config file maybe

        //TODO: Nothing bad should happen, however we might want to add error handling anyways
        let expires = Utc::now()
            .naive_utc()
            .checked_add_signed(Duration::days(1))
            .expect("Unable to create session");

        //TODO: This feels unsafe, maybe we should not pass the user id like this
        NewSession {
            user_id: user.id,
            token,
            expires
        }
    }

}

#[derive(Insertable)]
#[table_name="Users"]
pub struct NewUser {
    pub username: String,
    pub hash: String,
}

impl TryFrom<UserInfo> for NewUser {
    type Error = argon2::Error;

    fn try_from(user: UserInfo) -> Result<Self, Self::Error>{
        let mut rng = rand::thread_rng();
        let mut salt = vec![0; 128];

        rng.try_fill_bytes(&mut salt).unwrap();

        let mut config = argon2::Config::default();
        config.hash_length = 128;

        let hash = argon2::hash_encoded(user.password.as_ref(), &salt, &config)?;

        Ok(Self {
            username: user.name,
            hash,
        })
    }
}
#[derive(Clone, Debug, Deserialize)]
pub struct UserInfo {
    pub name: String,
    pub password: String,
}
impl fmt::Display for UserInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let b64 = format!("{}:{}", self.name, self.password);
        let result = base64::encode(&b64);
        write!(f, "{}", result)
    }
}

//TODO: Add error handling if string is malformed
impl From<String> for UserInfo {
    fn from(string: String) -> Self {
        let result:Vec<&str> = string.split(':').collect();

        Self {
            name: result[0].parse().unwrap(),
            password: result[1].parse().unwrap()
        }
    }
}

//TODO: Maybe sign the token or later include additional stuff