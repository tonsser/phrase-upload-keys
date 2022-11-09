extern crate reqwest;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
#[macro_use]
extern crate failure;
extern crate clap;
extern crate pbr;

use clap::{App, Arg, ArgMatches};
use failure::Error;
use pbr::ProgressBar;
use reqwest::Client;
use reqwest::Response;
use reqwest::StatusCode;
use std::fmt;
use std::fs::File;
use std::io::Read;

fn main() {
    let result = try_main();
    match result {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Error 💥:");
            eprintln!("{}", e);

            let backtrace = e.backtrace().to_string();
            if !backtrace.is_empty() {
                eprintln!("{}", backtrace);
            }
        }
    }
}

fn try_main() -> Result<(), Error> {
    let matches = arg_matches();

    let path: String = matches
        .value_of("FILE")
        .ok_or_else(|| CmdArgMissing { arg: "FILE".into() })?
        .to_string();

    let token = match matches.value_of("TOKEN") {
        Some(token) => Some(token.to_string()),
        None => std::env::var("PHRASE_ACCESS_TOKEN").ok(),
    }
    .map(Token)
    .ok_or_else(|| CmdArgMissing {
        arg: "access-token".into(),
    })?;

    let locale = matches.value_of("LOCALE").unwrap_or("en");

    let project_name: String = matches
        .value_of("PROJECT_NAME")
        .ok_or_else(|| CmdArgMissing {
            arg: "PROJECT_NAME".into(),
        })?
        .to_string();

    let keys = parse_keys(&path)?;

    let mut pb: Progress = ProgressBar::on(std::io::stderr(), keys.len() as u64);
    pb.format("╢▌▌░╟");

    upload_keys(keys, &project_name, &token, locale, &mut pb)?;

    pb.finish();

    Ok(())
}

fn arg_matches<'a>() -> ArgMatches<'a> {
    App::new("Phraseapp upload keys")
        .version("0.1")
        .author("David Pedersen <david@tonsser.com>")
        .about("Quickly upload multiple keys to Phrase")
        .arg(
            Arg::with_name("FILE")
                .help("Sets the input file to use")
                .required(true),
        ).arg(
            Arg::with_name("TOKEN")
                .short("t")
                .long("access-token")
                .help("The Phrase API token. Requires read and write scopes. Defaults to env var PHRASE_ACCESS_TOKEN if not given.")
                .takes_value(true)
                .required(false),
        ).arg(
            Arg::with_name("PROJECT_NAME")
                .short("p")
                .long("project-name")
                .help("The name of the Phrase project to add the strings to.")
                .takes_value(true)
                .required(true),
        ).arg(
            Arg::with_name("LOCALE")
                .short("l")
                .long("locale")
                .help("The locale the strings will be uploaded to. Defaults to en.")
                .takes_value(true)
                .required(false),
        ).get_matches()
}

type Progress = pbr::ProgressBar<std::io::Stderr>;

fn parse_keys(path: &str) -> Result<Vec<NewKey>, Error> {
    let contents = read_file(path)?;

    let mut keys = vec![];
    let mut strings = vec![];

    contents
        .lines()
        .filter(|line| !line.is_empty())
        .enumerate()
        .for_each(|(idx, line)| {
            if idx % 2 == 0 {
                keys.push(line);
            } else {
                strings.push(line);
            }
        });

    if keys.len() != strings.len() {
        Err(ParseError {
            file: path.to_string(),
        })?;
    }

    let new_keys = keys
        .into_iter()
        .zip(strings.into_iter())
        .map(|(key, string)| NewKey {
            key: key.to_string(),
            string: string.to_string(),
        })
        .collect();

    Ok(new_keys)
}

#[derive(Fail, Debug)]
#[fail(display = "Command line argument {} was missing", arg)]
pub struct CmdArgMissing {
    arg: String,
}

#[derive(Fail, Debug)]
#[fail(display = "Error parsing file {}", file)]
pub struct ParseError {
    file: String,
}

fn read_file(path: &str) -> Result<String, Error> {
    let mut f = File::open(path)?;
    let mut buf = String::new();
    f.read_to_string(&mut buf)?;
    Ok(buf)
}

fn upload_keys(
    keys: Vec<NewKey>,
    project_name: &str,
    token: &Token,
    locale: &str,
    pb: &mut Progress,
) -> Result<(), Error> {
    let client = Client::new();
    let project = find_project(&client, token, project_name)?;
    let locale = find_locale(&client, token, &project, locale)?;
    let created_keys = create_keys(&client, token, &project, keys)?;
    upload_translations(&client, token, &project, &locale, pb, created_keys)
}

#[derive(Deserialize, Debug)]
struct Project {
    id: String,
    name: String,
}

#[derive(Deserialize, Debug)]
struct Locale {
    id: String,
    name: String,
}

#[derive(Debug)]
struct NewKey {
    key: String,
    string: String,
}

#[derive(Deserialize, Debug)]
struct Key {
    id: String,
    name: String,
}

fn create_keys(
    client: &Client,
    token: &Token,
    project: &Project,
    keys: Vec<NewKey>,
) -> Result<Vec<(Key, String)>, Error> {
    let mut acc = vec![];
    for key in keys {
        acc.push(create_key(client, token, project, key)?);
    }
    Ok(acc)
}

fn create_key(
    client: &Client,
    token: &Token,
    project: &Project,
    key: NewKey,
) -> Result<(Key, String), Error> {
    let params = vec![("name".into(), key.key)];
    let translation = key.string;

    phrase_req(
        client,
        Method::Post(params),
        &format!("/api/v2/projects/{}/keys", project.id),
        token,
    )?
    .json()
    .map_err(Error::from)
    .map(|created_key| (created_key, translation))
}

fn upload_translations(
    client: &Client,
    token: &Token,
    project: &Project,
    locale: &Locale,
    pb: &mut Progress,
    keys: Vec<(Key, String)>,
) -> Result<(), Error> {
    for pair in keys {
        upload_translation(client, token, project, locale, pair)?;
        pb.inc();
    }
    Ok(())
}

fn upload_translation(
    client: &Client,
    token: &Token,
    project: &Project,
    locale: &Locale,
    key: (Key, String),
) -> Result<(), Error> {
    let params = vec![
        ("locale_id".into(), locale.id.clone()),
        ("key_id".into(), key.0.id),
        ("content".into(), key.1),
    ];

    phrase_req(
        client,
        Method::Post(params),
        &format!("/api/v2/projects/{}/translations", project.id),
        token,
    )
    .map(|_| ())
}

fn find_project(client: &Client, token: &Token, name: &str) -> Result<Project, Error> {
    let projects: Vec<Project> =
        phrase_req(client, Method::Get, "/api/v2/projects", token)?.json()?;

    projects
        .into_iter()
        .find(|p| p.name == name)
        .ok_or_else(|| ProjectNotFound { name: name.into() }.into())
}

fn find_locale(
    client: &Client,
    token: &Token,
    project: &Project,
    name: &str,
) -> Result<Locale, Error> {
    let projects: Vec<Locale> = phrase_req(
        client,
        Method::Get,
        &format!("/api/v2/projects/{}/locales", project.id),
        token,
    )?
    .json()?;

    projects
        .into_iter()
        .find(|p| p.name == name)
        .ok_or_else(|| LocaleNotFound { name: name.into() }.into())
}

fn phrase_req(
    client: &Client,
    method: Method,
    path: &str,
    token: &Token,
) -> Result<Response, Error> {
    let url = format!("https://api.phraseapp.com{}", path);

    let req = match &method {
        Method::Get => client.get(&url),
        Method::Post(ref params) => client.post(&url).form(&params),
    }
    .header(reqwest::header::AUTHORIZATION, format!("token {}", token.0));

    let resp = req.send()?;
    let status = resp.status();

    if status.is_success() {
        Ok(resp)
    } else {
        Err(RequestFailed {
            path: path.to_string(),
            method,
            status,
        }
        .into())
    }
}

struct Token(String);

#[derive(Debug, Clone)]
enum Method {
    Get,
    Post(Vec<(String, String)>),
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Method::Get => write!(f, "GET"),
            Method::Post(_) => write!(f, "POST"),
        }
    }
}

#[derive(Fail, Debug)]
#[fail(
    display = "Request to \"{} {}\" failed with status \"{}\"",
    method, path, status
)]
pub struct RequestFailed {
    path: String,
    method: Method,
    status: StatusCode,
}

#[derive(Fail, Debug)]
#[fail(display = "Phrase project named {} was not found", name)]
pub struct ProjectNotFound {
    name: String,
}

#[derive(Fail, Debug)]
#[fail(display = "Locale named {} was not found", name)]
pub struct LocaleNotFound {
    name: String,
}
