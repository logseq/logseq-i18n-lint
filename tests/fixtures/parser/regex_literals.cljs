;; Regex literals test fixtures

;; Basic regex
#"hello"

;; Regex with special chars
#"[a-z]+"
#"^\d{3}-\d{4}$"
#"\\w+"
#"\s+"

;; Regex with escaped quote
#"say \"hello\""

;; Regex should NOT be flagged as hardcoded string
(re-find #"[A-Z]+" input)
(re-matches #"^https?://" url)
