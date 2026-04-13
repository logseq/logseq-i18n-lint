;; UI function arguments
;; EXPECT: fn-arg-text

;; Should detect: hardcoded string in UI function call
(ui/button "Submit")
(ui/button "Cancel operation")
(shui/button {:label "Click me"})
(ui/icon "search")

;; Should NOT detect: translated
(ui/button (t :submit))
(shui/button {:label (t :click-me)})
