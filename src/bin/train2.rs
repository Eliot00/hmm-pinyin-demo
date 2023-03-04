use std::{collections::HashMap, fs::File, io::BufReader};

use redb::{Database, TableDefinition};
use serde_json::from_reader;

const INIT_TABLE: TableDefinition<&str, f64> = TableDefinition::new("init_prob");
const TRANS_TABLE: TableDefinition<(&str, &str), f64> = TableDefinition::new("trans_prob");
const EMISS_TABLE: TableDefinition<(&str, &str), f64> = TableDefinition::new("emiss_prob");
const PINYIN_STATES: TableDefinition<&str, &str> = TableDefinition::new("pinyin_states");

fn main() {
    let db = Database::create("hmm.redb").unwrap();

    let init_file = File::open("./params/init_prob.json").unwrap();
    let reader = BufReader::new(init_file);
    let init_map: HashMap<String, f64> = from_reader(reader).unwrap();

    let trans_file = File::open("./params/trans_prob.json").unwrap();
    let reader = BufReader::new(trans_file);
    let trans_map: HashMap<String, HashMap<String, f64>> = from_reader(reader).unwrap();

    let emission_file = File::open("./params/emiss_prob.json").unwrap();
    let reader = BufReader::new(emission_file);
    let emiss_map: HashMap<String, HashMap<String, f64>> = from_reader(reader).unwrap();

    let pinyin_file = File::open("./params/pinyin_states.json").unwrap();
    let reader = BufReader::new(pinyin_file);
    let pinyin_map: HashMap<String, Vec<String>> = from_reader(reader).unwrap();

    let write_txn = db.begin_write().unwrap();
    {
        let mut table = write_txn.open_table(INIT_TABLE).unwrap();
        for (key, value) in init_map {
            table.insert(&key.as_str(), value).unwrap();
        }

        let mut table = write_txn.open_table(TRANS_TABLE).unwrap();
        for (word, value) in trans_map {
            for (pre, prob) in value {
                table.insert((word.as_str(), pre.as_str()), prob).unwrap();
            }
        }

        let mut table = write_txn.open_table(EMISS_TABLE).unwrap();
        for (word, value) in emiss_map {
            for (pinyin, prob) in value {
                table
                    .insert((word.as_str(), pinyin.as_str()), prob)
                    .unwrap();
            }
        }

        let mut table = write_txn.open_table(PINYIN_STATES).unwrap();
        for (word, pinyin_list) in pinyin_map {
            table
                .insert(word.as_str(), pinyin_list.join("").as_str())
                .unwrap();
        }
    }
    write_txn.commit().unwrap();
}
