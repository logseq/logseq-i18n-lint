;; String concatenation in UI context
;; EXPECT: str-concat

;; Should detect: hardcoded strings in str calls within hiccup
[:div (str "Error: " error-msg)]
[:span (str "Hello " user-name "!")]

;; Should NOT detect: str outside UI context
(str "data-" id)
(str "/" path)

;; Should NOT detect: translated
[:div (str (t :error-prefix) error-msg)]
