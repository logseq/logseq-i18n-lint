;; Conditional text
;; EXPECT: conditional-text

[:div (if loading? "Loading..." "Ready to start")]
[:span (when error "Something went wrong")]
[:p (case status
  :success "Operation completed"
  :error "An error occurred"
  "Unknown status")]
[:span (if-not loaded? "Loading..." "Loaded")]
[:div (or label "Untitled")]

;; Should NOT detect: translated
(if loading? (t :loading) (t :ready))

;; Should NOT detect: outside UI context
(if loading? "Loading..." "Ready to start")
(if-not loaded? "Loading..." "Loaded")
(or label "Untitled")
(throw (ex-info "validation failed" {:type :error}))
