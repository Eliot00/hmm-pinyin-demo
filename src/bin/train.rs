use std::f64::consts::E;
use std::{collections::HashMap, fs::File, io::Read};

use pinyin::ToPinyin;
use redb::{Database, ReadableTable, TableDefinition};
use regex::Regex;

const INIT_TABLE: TableDefinition<&str, f64> = TableDefinition::new("init_prob");
const TRANS_TABLE: TableDefinition<(&str, &str), f64> = TableDefinition::new("trans_prob");
const EMISS_TABLE: TableDefinition<(&str, &str), f64> = TableDefinition::new("emiss_prob");
const PINYIN_STATES: TableDefinition<&str, &str> = TableDefinition::new("pinyin_states");

fn main() {
    let chinese_re = Regex::new(r#"[\u4e00-\u9fa5]{2,}"#).unwrap();
    let mut file = File::open("./2014_corpus_pre.txt").unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();

    let mut seqs = Vec::new();
    for seq in chinese_re.find_iter(&contents) {
        seqs.push(seq.as_str().to_string());
    }

    let db = Database::create("hmm.redb").unwrap();
    count_init(&db, &seqs);
    count_trans(&db, &seqs);
    count_emission(&db, &seqs);
    count_pinyin_states(&db);
}

fn count_init(db: &Database, seqs: &Vec<String>) {
    let mut temp_table: HashMap<String, u64> = HashMap::new();
    let mut num = 0;
    let len = seqs.len();

    for seq in seqs {
        num += 1;
        if num % 10000 == 0 {
            println!("{}/{}", num, len);
            println!("{}", seq);
        }
        if seq.is_empty() {
            continue;
        }

        let first_char = seq.chars().next().unwrap().to_string();
        println!("first_char: {}", first_char);
        if let Some(value) = temp_table.get_mut(&first_char) {
            *value = value.to_owned() + 1;
        } else {
            temp_table.insert(first_char, 1);
        }
    }

    println!("init map: {:?}", temp_table);

    let write_txn = db.begin_write().unwrap();
    {
        let mut table = write_txn.open_table(INIT_TABLE).unwrap();
        for (key, value) in temp_table {
            let value = (value as f64 / len as f64).log(E);
            table.insert(&key.as_str(), value).unwrap();
        }
    }
    write_txn.commit().unwrap();
}

fn count_trans(db: &Database, seqs: &Vec<String>) {
    let mut temp: HashMap<String, HashMap<String, u64>> = HashMap::new();
    let mut num = 0;
    let len = seqs.len();

    for seq in seqs {
        num += 1;
        if num % 10000 == 0 {
            println!("{}/{}", num, len);
        }
        if seq.is_empty() {
            continue;
        }

        let mut chars: Vec<String> = seq.chars().map(|c| c.to_string()).collect();
        chars.insert(0, "BOS".to_string());
        chars.push("EOS".to_string());

        for (index, post) in chars.iter().enumerate() {
            if index == 0 {
                continue;
            }

            let pre = chars[index - 1].clone();
            if temp.get(post.as_str()).is_none() {
                temp.insert(post.to_owned(), HashMap::new());
            }
            let key = temp.get_mut(post.as_str()).unwrap();
            let pre_ = pre.clone();
            (*key).insert(pre, key.get(pre_.as_str()).unwrap_or(&0).to_owned() + 1);
        }
    }

    let write_txn = db.begin_write().unwrap();
    {
        let mut table = write_txn.open_table(TRANS_TABLE).unwrap();
        for (post, value) in temp {
            let total = value.values().sum::<u64>();
            for (pre, count) in value {
                let prob = (count as f64 / total as f64).log(E);
                table.insert((post.as_str(), pre.as_str()), prob).unwrap();
            }
        }
    }
    write_txn.commit().unwrap();
}

fn count_emission(db: &Database, seqs: &Vec<String>) {
    let mut temp: HashMap<String, HashMap<String, u64>> = HashMap::new();
    let mut num = 0;
    let len = seqs.len();

    for seq in seqs {
        num += 1;
        if num % 10000 == 0 {
            println!("{}/{}", num, len);
        }
        if seq.is_empty() {
            continue;
        }

        let pinyin = seq.as_str().to_pinyin();
        let zip_iter = pinyin.zip(seq.chars());
        for (py, word) in zip_iter {
            if temp.get(word.to_string().as_str()).is_none() {
                temp.insert(word.to_string(), HashMap::new());
            }
            let key = temp.get_mut(word.to_string().as_str()).unwrap();
            let py_str = py.unwrap().plain();
            (*key).insert(
                py_str.to_string(),
                key.get(py_str).unwrap_or(&0).to_owned() + 1,
            );
        }
    }

    let write_txn = db.begin_write().unwrap();
    {
        let mut table = write_txn.open_table(EMISS_TABLE).unwrap();
        for (word, pinyins) in temp {
            let total = pinyins.values().sum::<u64>();
            for (py, count) in pinyins {
                let prob = (count as f64 / total as f64).log(E);
                table.insert((word.as_str(), py.as_str()), prob).unwrap();
            }
        }
    }
    write_txn.commit().unwrap();
}

fn count_pinyin_states(db: &Database) {
    let read_txn = db.begin_read().unwrap();
    let write_txn = db.begin_write().unwrap();
    {
        let emission_table = read_txn.open_table(EMISS_TABLE).unwrap();
        let mut pinyin_states_table = write_txn.open_table(PINYIN_STATES).unwrap();
        for (key, _) in emission_table.iter().unwrap() {
            let (word, py) = key.value().to_owned();
            let mut words = pinyin_states_table
                .get(py)
                .unwrap()
                .map(|x| x.value().to_string())
                .unwrap_or("".to_string());
            words.push_str(word);
            pinyin_states_table.insert(py, words.as_str()).unwrap();
        }
    }
    write_txn.commit().unwrap();
}
