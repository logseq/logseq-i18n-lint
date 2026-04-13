;; Reader macros test fixtures

;; Anonymous function
#(+ %1 %2)

;; Deref
@my-atom
@(subscribe [:some-key])

;; Quote
'foo
'(1 2 3)

;; Syntax quote
`(let [x# 1] x#)

;; Unquote
~expr
~@exprs

;; Var quote
#'my-var

;; Metadata
^:private my-fn
^{:doc "example"} my-var

;; Reader conditional
#?(:clj "java" :cljs "javascript")
#?@(:clj [1 2] :cljs [3 4])

;; Tagged literal
#inst "2024-01-01"
#uuid "550e8400-e29b-41d4-a716-446655440000"

;; Old-style metadata
#^{:deprecated true} old-fn
