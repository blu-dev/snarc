use std::{convert::Infallible, fmt, path::Path, str::FromStr};

use hash40::Hash40;

use crate::HASH_MASK;

pub mod table;

pub mod packaged;
pub mod search;
pub mod stream;

use binrw::{BinRead, BinWrite, VecArgs};

#[derive(Debug, Copy, Clone, PartialEq, Eq, BinRead, BinWrite)]
pub struct HashKey(u64);

impl HashKey {
    pub fn new(hash: Hash40, index: usize) -> Self {
        Self(hash.0 | (index as u64) << 40)
    }

    pub fn hash(self) -> Hash40 {
        Hash40(self.0 & HASH_MASK)
    }

    pub fn index(self) -> usize {
        (self.0 & !HASH_MASK) as usize >> 40
    }

    pub fn set_hash(&mut self, hash: Hash40) {
        self.0 = (self.0 & !HASH_MASK) | hash.0;
    }

    pub fn set_index(&mut self, index: usize) {
        self.0 = (self.0 & HASH_MASK) | (index as u64) << 40;
    }
}

impl fmt::Display for HashKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#08x} | {}", self.index(), self.hash())
    }
}

pub(crate) fn read_table<T: BinRead>(path: &Path, item_size: usize) -> binrw::BinResult<Vec<T>>
where
    <T as BinRead>::Args: Default,
{
    let bytes = std::fs::read(path)?;
    if bytes.len() % item_size != 0 {
        return Err(binrw::Error::Custom {
            pos: 0,
            err: Box::new(
                "Table alignment error: byte length of table is not a multiple of item_size",
            ),
        });
    }
    let count = bytes.len() / item_size;
    Vec::read_args(
        &mut std::io::Cursor::new(bytes),
        VecArgs::builder().count(count).finalize(),
    )
}

#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Locale {
    Invalid = -1,
    Japanese = 0,
    UsEnglish,
    UsFrench,
    UsSpanish,
    EuEnglish,
    EuFrench,
    EuSpanish,
    German,
    Dutch,
    Italian,
    Russian,
    Korean,
    Chinese,
    Taiwanese,
}

impl Locale {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Japanese => "jp_ja",
            Self::UsEnglish => "us_en",
            Self::UsFrench => "us_fr",
            Self::UsSpanish => "us_es",
            Self::EuEnglish => "eu_en",
            Self::EuFrench => "eu_fr",
            Self::EuSpanish => "eu_es",
            Self::German => "eu_de",
            Self::Dutch => "eu_nl",
            Self::Italian => "eu_it",
            Self::Russian => "eu_ru",
            Self::Korean => "kr_ko",
            Self::Chinese => "zh_cn",
            Self::Taiwanese => "zh_tw",
            Self::Invalid => "",
        }
    }

    pub fn as_pretty_str(self) -> &'static str {
        match self {
            Self::Japanese => "Japanese",
            Self::UsEnglish => "US English",
            Self::UsFrench => "US French",
            Self::UsSpanish => "US Spanish",
            Self::EuEnglish => "EU English",
            Self::EuFrench => "EU French",
            Self::EuSpanish => "EU Spanish",
            Self::German => "German",
            Self::Dutch => "Dutch",
            Self::Italian => "Italian",
            Self::Russian => "Russian",
            Self::Korean => "Korean",
            Self::Chinese => "Chinese",
            Self::Taiwanese => "Taiwanese",
            Self::Invalid => "",
        }
    }
}

impl FromStr for Locale {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let locale = match s {
            "jp_ja" => Self::Japanese,
            "us_en" => Self::UsEnglish,
            "us_fr" => Self::UsFrench,
            "us_es" => Self::UsSpanish,
            "eu_en" => Self::UsEnglish,
            "eu_fr" => Self::UsFrench,
            "eu_es" => Self::UsSpanish,
            "eu_de" => Self::German,
            "eu_nl" => Self::Dutch,
            "eu_it" => Self::Italian,
            "eu_ru" => Self::Russian,
            "kr_ko" => Self::Korean,
            "zh_cn" => Self::Chinese,
            "zh_tw" => Self::Taiwanese,
            _ => Self::Invalid,
        };

        Ok(locale)
    }
}

impl From<i32> for Locale {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Japanese,
            1 => Self::UsEnglish,
            2 => Self::UsFrench,
            3 => Self::UsSpanish,
            4 => Self::EuEnglish,
            5 => Self::EuFrench,
            6 => Self::EuSpanish,
            7 => Self::German,
            8 => Self::Dutch,
            9 => Self::Italian,
            10 => Self::Russian,
            11 => Self::Korean,
            12 => Self::Chinese,
            13 => Self::Taiwanese,
            _ => Self::Invalid,
        }
    }
}

#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Region {
    Invalid = -1,
    Japan = 0,
    NorthAmerica,
    Europe,
    Korea,
    China,
}

impl From<i32> for Region {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Japan,
            1 => Self::NorthAmerica,
            2 => Self::Europe,
            3 => Self::Korea,
            4 => Self::China,
            _ => Self::Invalid,
        }
    }
}

impl From<Locale> for Region {
    fn from(value: Locale) -> Self {
        match value {
            Locale::Japanese => Self::Japan,
            Locale::UsEnglish | Locale::UsFrench | Locale::UsSpanish => Self::NorthAmerica,
            Locale::EuEnglish
            | Locale::EuFrench
            | Locale::EuSpanish
            | Locale::German
            | Locale::Dutch
            | Locale::Italian
            | Locale::Russian => Self::Europe,
            Locale::Korean => Self::Korea,
            Locale::Chinese | Locale::Taiwanese => Self::China,
            Locale::Invalid => Self::Invalid,
        }
    }
}

impl FromStr for Region {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let region = match s {
            "jp" => Self::Japan,
            "us" => Self::NorthAmerica,
            "eu" => Self::Europe,
            "kr" => Self::Korea,
            "zh" => Self::China,
            _ => Self::Invalid,
        };

        Ok(region)
    }
}

impl Region {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Japan => "jp",
            Self::NorthAmerica => "us",
            Self::Europe => "eu",
            Self::Korea => "kr",
            Self::China => "zh",
            Self::Invalid => "",
        }
    }

    pub fn as_pretty_str(self) -> &'static str {
        match self {
            Self::Japan => "Japan",
            Self::NorthAmerica => "North America",
            Self::Europe => "Europe",
            Self::Korea => "Korea",
            Self::China => "China",
            Self::Invalid => "",
        }
    }
}
