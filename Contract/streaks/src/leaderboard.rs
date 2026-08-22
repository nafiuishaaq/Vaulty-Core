use shared::errors::Error;
use shared::storage::StorageHelper;
use soroban_sdk::{Address, BytesN, Env, Vec};

pub struct Leaderboard;

impl Leaderboard {
    fn leaderboard_key(env: &Env) -> BytesN<32> {
        BytesN::from_array(env, &[9u8; 32])
    }

    pub fn get_leaderboard(env: &Env) -> Result<Vec<(Address, u32)>, Error> {
        let key = Self::leaderboard_key(env);
        let board: Vec<(Address, u32)> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(env));
        StorageHelper::touch_streak(env, &key);
        Ok(board)
    }

    pub fn update_leaderboard(env: &Env, user: &Address, streak: u32) -> Result<(), Error> {
        let key = Self::leaderboard_key(env);
        let mut board: Vec<(Address, u32)> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(env));

        let mut found = false;
        for i in 0..board.len() {
            if let Some((entry_user, _)) = board.get(i) {
                if entry_user == *user {
                    board.set(i, (user.clone(), streak));
                    found = true;
                    break;
                }
            }
        }

        if !found {
            board.push_back((user.clone(), streak));
        }

        env.storage().persistent().set(&key, &board);
        StorageHelper::touch_streak(env, &key);
        Ok(())
    }
}
