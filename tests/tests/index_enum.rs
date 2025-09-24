use tests::{models, tests, DbTest};
use toasty::{self, stmt::Id, ToastyEnum};

async fn index_enum(test: &mut DbTest) {
    #[derive(toasty::Model)]
    struct LogEntry {
        #[key]
        #[auto]
        request_id: Id<Self>,

        #[index]
        log_level: LogLevel,

        message: String,
    }

    #[derive(Debug, PartialEq, ToastyEnum, Clone)]
    enum LogLevel {
        Debug,
        Info,
        Warn,
        Error,
    }

    let db = test.setup_db(models!(LogEntry)).await;

    {
        use LogLevel::{Debug, Error, Warn};
        for (log_level, message) in [
            (Debug, "initializing"),
            (Warn, "something fishy"),
            (Error, "null pointer"),
        ] {
            LogEntry::create()
                .log_level(log_level)
                .message(message)
                .exec(&db)
                .await
                .expect("failed to create entry");
        }

        let res = LogEntry::filter_by_log_level(Warn).all(&db).await.unwrap();
        let entries = res.collect::<Vec<LogEntry>>().await.unwrap();
        assert_eq!(
            vec![(Warn, "something fishy".to_string())],
            entries
                .iter()
                .map(|le| (le.log_level.clone(), le.message.clone()))
                .collect::<Vec<(LogLevel, String)>>()
        );
    }
}

tests!(index_enum);
