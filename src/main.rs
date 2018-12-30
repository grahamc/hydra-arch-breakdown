
#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;

use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::collections::HashMap;

use std::fmt;

macro_rules! enum_number {
    ($name:ident { $($variant:ident = $value:expr, )* }) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum $name {
            $($variant = $value,)*
        }

        impl ::serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                // Serialize the enum as a u64.
                serializer.serialize_u64(*self as u64)
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: ::serde::Deserializer<'de>,
            {
                struct Visitor;

                impl<'de> ::serde::de::Visitor<'de> for Visitor {
                    type Value = $name;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("positive integer")
                    }

                    fn visit_u64<E>(self, value: u64) -> Result<$name, E>
                    where
                        E: ::serde::de::Error,
                    {
                        // Rust does not come with a simple way of converting a
                        // number to an enum, so use a big `match`.
                        match value {
                            $( $value => Ok($name::$variant), )*
                            _ => Err(E::custom(
                                format!("unknown {} value: {}",
                                stringify!($name), value))),
                        }
                    }
                }

                // Deserialize the enum from a u64.
                deserializer.deserialize_u64(Visitor)
            }
        }
    }
}


#[derive(Deserialize, Debug)]
struct Build<'a> {
    buildstatus: Option<BuildStatus>,
    job: &'a str,
    system: &'a str,
    nixname: &'a str,
    id: u64,
}

enum_number!(BuildStatus {
    // See https://github.com/NixOS/hydra/blob/master/src/sql/hydra.sql#L202-L215
    Success = 0,
    BuildFailed = 1,
    DependencyFailed = 2,
    HostFailureAbort = 3,
    Cancelled = 4,
    // Obsolete
    FailureWithOutput = 6,
    TimeOut = 7,
    CachedFailure = 8,
    UnsupportedSystem = 9,
    LogLimitExceeded = 10,
    OutputLimitExceeded = 11,
    NotDeterministic = 12,
});


fn main() {
    let file = File::open("latest-eval-builds").unwrap();
    let mut buf_reader = BufReader::new(file);

    let mut contents = String::new();
    buf_reader.read_to_string(&mut contents).unwrap();

    let builds: Vec<Build> = serde_json::from_str::<Vec<Build>>(&contents).unwrap()
        .into_iter()
        .filter(|build| build.system != "x86_64-darwin" && build.system != "i686-linux")
        .collect();

    let by_nixname: HashMap<&str, Vec<&Build>> = builds
        .iter()
        .fold(HashMap::new(), |mut acc, build| {
            if ! acc.contains_key(build.nixname) {
                acc.insert(build.nixname, vec![]);
            } else {
                acc.get_mut(build.nixname).unwrap().push(build);
            }
            return acc;
        });

    let ok_all_platforms: u32 = by_nixname
        .iter()
        .filter(|(_nixname, builds)| {
            builds
                .iter()
                .fold(true, |acc, build| acc && build.buildstatus == Some(BuildStatus::Success))
        })
        .fold(0, |acc, _| acc + 1);

    let not_ok_all_platforms: HashMap<&&str, HashMap<&str, &&Build>> = by_nixname
        .iter()
        .filter(|(_nixname, builds)| {
            ! builds
                .iter()
                .fold(true, |acc, build| acc && build.buildstatus == Some(BuildStatus::Success))
        })
        .map(|(nixname, build_vec)| {
            (
                nixname,
                build_vec
                    .iter()
                    .map(|build| (build.system, build))
                    .collect::<HashMap<&str, &&Build>>()
            )
        })
        .collect();

    let max_nix_name_len: usize = not_ok_all_platforms
        .iter()
        .fold(0, |acc, (nixname, _)|
              {
                  let len = nixname.len();
                  if len > acc {
                      return len;
                  } else {
                      return acc;
                  }
              }
        );

    println!("builds fine on all platforms: {:?}", ok_all_platforms);

    for (job, builds) in not_ok_all_platforms {
        match (builds.get("x86_64-linux"), builds.get("aarch64-linux")) {
            (Some(x86_64), Some(aarch64)) => {
                match (x86_64.buildstatus, aarch64.buildstatus) {
                    (Some(BuildStatus::Success), Some(BuildStatus::Success)) => {},
                    (Some(BuildStatus::Success), aarch64_status) => {
                        print!("{:>width$} https://hydra.nixos.org/build/{} arch64 status: {:?}", job, aarch64.id, aarch64_status, width = max_nix_name_len,
                        );
                        println!("");
                    },
                    (_, _) => {}
                }
            }
            _ => {}
        }
    }
}
