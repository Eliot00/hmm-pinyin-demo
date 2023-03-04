use std::{collections::HashMap, io};

use itertools::{iproduct, Itertools};
use redb::{Database, ReadableTable, TableDefinition};

const INIT_TABLE: TableDefinition<&str, f64> = TableDefinition::new("init_prob");
const TRANS_TABLE: TableDefinition<(&str, &str), f64> = TableDefinition::new("trans_prob");
const EMISS_TABLE: TableDefinition<(&str, &str), f64> = TableDefinition::new("emiss_prob");
const PINYIN_STATES: TableDefinition<&str, &str> = TableDefinition::new("pinyin_states");

fn main() {
    let db = Database::create("hmm.redb").unwrap();
    let hmm = HMM::new(db);

    let mut input = String::new();
    match io::stdin().read_line(&mut input) {
        Ok(_) => {
            hmm.trans(input.trim())
                .iter()
                .enumerate()
                .for_each(|(i, result)| {
                    println!("result{}: {}", i, result);
                });
        }
        Err(error) => println!("error: {}", error),
    }
}

struct HMM {
    py_list: Vec<String>,
    db: Database,
}

impl HMM {
    fn new(db: Database) -> Self {
        let sm_list = "b,p,m,f,d,t,n,l,g,k,h,j,q,x,z,c,s,r,zh,ch,sh,y,w".split(",");
        let ym_list = "a,o,e,i,u,v,ai,ei,ui,ao,ou,iu,ie,ve,er,an,en,in,un,ang,eng,ing,ong,uai,ia,uan,uang,uo,ua".split(",");
        let ztrd_list = "a,o,e,ai,ei,ao,ou,er,an,en,ang,zi,ci,si,zhi,chi,shi,ri,yi,wu,yu,yin,ying,yun,ye,yue,yuan".split(",");
        let mut py_list = Vec::new();
        for (s, y) in iproduct!(sm_list, ym_list) {
            let temp = s.to_string() + y;
            if !py_list.contains(&temp) {
                py_list.push(temp);
            }
        }

        for z in ztrd_list {
            if !py_list.contains(&z.to_string()) {
                py_list.push(z.to_string());
            }
        }

        Self { db, py_list }
    }

    fn trans(&self, code: &str) -> Vec<String> {
        let seq = pysplict(code, &self.py_list);
        let min_f = -3.14e100;
        let mut result: Vec<(usize, String)> = Vec::new();

        let read_txn = self.db.begin_read().unwrap();
        let init_prob = read_txn.open_table(INIT_TABLE).unwrap();
        let pinyin_states = read_txn.open_table(PINYIN_STATES).unwrap();
        let trans_table = read_txn.open_table(TRANS_TABLE).unwrap();
        let emiss_prob = read_txn.open_table(EMISS_TABLE).unwrap();

        for n in 0..(seq.len()) {
            let length = seq[n].len();
            let mut viterbi: HashMap<usize, HashMap<String, (f64, String)>> = HashMap::new();
            for i in 0..length {
                viterbi.insert(i, HashMap::new());
            }

            let key = seq[n][0].as_str();
            let chars = pinyin_states.get(key).unwrap().unwrap();
            for s in chars.value().chars() {
                let p = viterbi.get_mut(&0).unwrap();
                let init = init_prob
                    .get(s.to_string().as_str())
                    .unwrap()
                    .map(|x| x.value())
                    .unwrap_or(min_f);
                let emiss = emiss_prob
                    .get((s.to_string().as_str(), seq[n][0].as_str()))
                    .unwrap()
                    .map(|x| x.value())
                    .unwrap_or(min_f);
                p.insert(s.to_string(), (init + emiss, "".to_string()));
            }

            for i in 0..(length - 1) {
                let key = seq[n][i + 1].as_str();
                let chars = pinyin_states.get(key).unwrap().unwrap();
                for s in chars.value().chars() {
                    let value = pinyin_states
                        .get(seq[n][i].as_str())
                        .unwrap()
                        .unwrap()
                        .value()
                        .chars()
                        .map(|c| {
                            let vit = viterbi[&i][c.to_string().as_str()].0;
                            let emission = emiss_prob
                                .get((s.to_string().as_str(), seq[n][i + 1].as_str()))
                                .unwrap()
                                .map(|e| e.value())
                                .unwrap_or(min_f);
                            let trans = trans_table
                                .get((s.to_string().as_str(), c.to_string().as_str()))
                                .unwrap()
                                .map(|t| t.value())
                                .unwrap_or(min_f);
                            (vit + emission + trans, c.to_string())
                        })
                        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
                        .unwrap();
                    let p = viterbi.get_mut(&(i + 1)).unwrap();
                    p.insert(s.to_string(), value);
                }
            }

            let key = seq[n].last().unwrap().as_str();
            let last = pinyin_states.get(key).unwrap().unwrap();
            for s in last.value().chars() {
                let old = &viterbi[&(length - 1)][s.to_string().as_str()];
                let trans = trans_table
                    .get(("EOS", s.to_string().as_str()))
                    .unwrap()
                    .map(|x| x.value())
                    .unwrap_or(min_f);
                let new_value = (old.0 + trans, old.1.clone());
                let p = viterbi.get_mut(&(length - 1)).unwrap();
                p.insert(s.to_string(), new_value);
            }

            let words_list = viterbi[&(length - 1)]
                .iter()
                .sorted_by(|a, b| a.1 .0.partial_cmp(&b.1 .0).unwrap())
                .rev()
                .take(100);

            for (idx, data) in words_list.enumerate() {
                let mut words = vec!["".to_string(); length];
                if let Some(last) = words.last_mut() {
                    *last = data.0.clone();
                }

                for n in (0..(length - 1)).rev() {
                    words[n] = viterbi[&(n + 1)][words[n + 1].to_string().as_str()]
                        .1
                        .clone();
                }

                result.push((idx, words.join("")));
            }
        }

        println!("{:?}", result);

        result.sort_by_key(|x| x.0);
        result.iter().map(|x| x.1.clone()).collect_vec()
    }
}

fn pysplict(word: &str, word_list: &Vec<String>) -> Vec<Vec<String>> {
    let mut res = Vec::new();
    dp(&mut res, word, word_list, "".to_string());
    res
}

fn dp(res: &mut Vec<Vec<String>>, word: &str, word_list: &Vec<String>, pinyin_list_str: String) {
    let len = word.len();
    for i in 0..=len {
        let mut p_list: Vec<String> = pinyin_list_str.split(",").map(|x| x.to_string()).collect();
        let sub_word = word[0..i].to_string();
        if word_list.contains(&sub_word) {
            if i == len {
                p_list.push(sub_word);
                res.push(p_list[1..].iter().map(|x| x.to_string()).collect());
            } else {
                p_list.push(sub_word);
                dp(res, &word[i..], word_list, p_list.join(","));
            }
        }
    }
}
