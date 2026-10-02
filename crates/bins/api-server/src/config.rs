            Environment::Development
        );
        assert_eq!(Environment::parse("test").unwrap(), Environment::Test);
        assert_eq!(
            Environment::parse("production").unwrap(),
            Environment::Production
        );

        assert!(Environment::parse("prod").is_err());
        assert!(Environment::parse("staging").is_err());
        assert!(Environment::parse("PRODUCTION").is_err());
        assert!(Environment::parse("").is_err());
    }

    #[test]
    fn production_invariants_require_all_security_prerequisites() {
        let missing_everything = AppConfig::validate_invariants(
            Environment::Production,
            "sqlite::memory:",
            false,
            false,
            false,
        );
        assert!(missing_everything.is_err());

        let missing_governance = AppConfig::validate_invariants(
            Environment::Production,
            "postgres://db",
            false,
            true,
            true,
        );
        assert!(missing_governance.is_err());

        let missing_signing_key = AppConfig::validate_invariants(
            Environment::Production,
            "postgres://db",
            true,
            false,
            true,
        );
        assert!(missing_signing_key.is_err());

        let missing_cors = AppConfig::validate_invariants(
            Environment::Production,
            "postgres://db",
            true,
            true,
            false,
        );
        assert!(missing_cors.is_err());

        assert!(AppConfig::validate_invariants(
            Environment::Production,
            "postgres://db",
            true,
            true,
            true,
        )
        .is_ok());
    }

    #[test]
    fn non_production_environments_keep_local_compositions_possible() {
        assert!(AppConfig::validate_invariants(
            Environment::Development,
            "sqlite::memory:",
            false,
            false,
            false,
        )
        .is_ok());
        assert!(AppConfig::validate_invariants(
            Environment::Test,
            "sqlite::memory:",
            false,
            false,
            false,
        )
        .is_ok());
    }
}
