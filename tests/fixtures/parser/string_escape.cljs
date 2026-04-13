;; String escape sequences test fixtures

;; Basic escapes
"hello\nworld"
"tab\there"
"return\rcarriage"
"quote\"inside"
"backslash\\"

;; Unicode escapes
"\u0041"           ;; A
"\u4F60\u597D"     ;; 你好
"\u00E9"           ;; é

;; Empty string
""

;; String with only spaces
"   "

;; Multi-line string content
"line1\nline2\nline3"
