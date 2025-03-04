use super::Image;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum ActivityPeriod {
    Timespan {
        start_year: u16,
        end_year: Option<u16>,
    },
    Decade(u16),
}

#[derive(Clone, Deserialize, Serialize, Debug, PartialEq, Eq)]
pub struct Artist {
    pub id: String,
    pub name: String,
    pub genre: Vec<String>,
    pub portraits: Vec<Image>,
    pub activity_periods: Vec<ActivityPeriod>,
    pub related: Vec<String>,
    pub portrait_group: Vec<Image>,
}

#[derive(Clone, Serialize)]
pub struct ArtistDiscography {
    pub albums: Vec<String>,
    pub features: Vec<String>,
}

#[cfg(test)]
mod tests {
    use crate::catalog::image::ImageSize;

    use super::*;

    #[test]
    fn parses_activity_period1() {
        let s = "
        {
            \"Timespan\": {
                \"start_year\": 1989,
                \"end_year\": null
             }
        }
        ";
        let expected = ActivityPeriod::Timespan {
            start_year: 1989,
            end_year: None,
        };
        match serde_json::from_str::<ActivityPeriod>(s) {
            Ok(x) => assert_eq!(x, expected),
            Err(_) => assert!(false, "Did not parse json string."),
        }
    }

    #[test]
    fn parses_activity_period2() {
        let s = "
        {
            \"Timespan\": {
                \"start_year\": 1989,
                \"end_year\": 1999
             }
        }
        ";
        let expected = ActivityPeriod::Timespan {
            start_year: 1989,
            end_year: Some(1999),
        };
        match serde_json::from_str::<ActivityPeriod>(s) {
            Ok(x) => assert_eq!(x, expected),
            Err(_) => assert!(false, "Did not parse json string."),
        }
    }

    #[test]
    fn parses_activity_period3() {
        let s = "
        {
            \"Decade\": 1990
        }
        ";
        let expected = ActivityPeriod::Decade(1990);

        match serde_json::from_str::<ActivityPeriod>(s) {
            Ok(x) => assert_eq!(x, expected),
            Err(_) => assert!(false, "Did not parse json string."),
        }
    }

    #[test]
    fn parses_artist1() {
        let s = r#"
        {
            "id": "5PF3HYijywmkoIgVSwXtP8",
            "name": "Emily Muli",
            "genre": [],
            "portraits": [],
            "activity_periods": [],
            "related": [
              "2oj4UeLdDJlU7EdgoaBylD",
              "5ZQF36w4zKY03Rq4zbYx88"
            ],
            "portrait_group": [
              {
                "id": "ab676161000051747e0c5c965bdf72e29300961b",
                "size": "DEFAULT",
                "width": 320,
                "height": 320
              },
              {
                "id": "ab6761610000f1787e0c5c965bdf72e29300961b",
                "size": "SMALL",
                "width": 160,
                "height": 80
              }
            ]
          }
        "#;
        let expected = Artist {
            id: "5PF3HYijywmkoIgVSwXtP8".to_owned(),
            name: "Emily Muli".to_owned(),
            genre: vec![],
            portraits: vec![],
            activity_periods: vec![],
            related: vec![
                "2oj4UeLdDJlU7EdgoaBylD".to_owned(),
                "5ZQF36w4zKY03Rq4zbYx88".to_owned(),
            ],
            portrait_group: vec![
                Image {
                    id: "ab676161000051747e0c5c965bdf72e29300961b".to_owned(),
                    size: ImageSize::DEFAULT,
                    width: 320,
                    height: 320,
                },
                Image {
                    id: "ab6761610000f1787e0c5c965bdf72e29300961b".to_owned(),
                    size: ImageSize::SMALL,
                    width: 160,
                    height: 80,
                },
            ],
        };

        match serde_json::from_str::<Artist>(s) {
            Ok(x) => assert_eq!(x, expected),
            Err(_) => assert!(false, "Did not parse json string."),
        }
    }
}
