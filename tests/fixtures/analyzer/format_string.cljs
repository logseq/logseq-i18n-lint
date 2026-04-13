;; Format string detection
;; EXPECT: format-string for calls inside UI context only

;; Should detect: format inside hiccup
[:div (format "Found %d items" count)]
[:span (goog.string/format "Hello, %s!" name)]

;; Should NOT detect: format outside UI context
(format "log: %s" value)
(goog.string/format "key-%s" id)

;; Should NOT detect: translated format template
[:div (format (t :items-count) count)]

;; Should NOT detect: format in a non-UI function body
(defn make-key [n]
  (format "item-%d" n))
