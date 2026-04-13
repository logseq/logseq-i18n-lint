;; Comments test fixtures

;; Line comment
; this is a comment
;; double semicolon comment

;; Discard
#_ (this form is discarded)
#_ "discarded string"

;; Comment form
(comment
  (println "this is in comment")
  [:div "also in comment"])

;; Inline after expression
(def x 42) ; trailing comment
