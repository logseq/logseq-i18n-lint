;; Regression test: anonymous function that directly returns hiccup.
;; EXPECT: hiccup-text for "Hidden in fn" and "Multi-body return"
;; EXPECT: hiccup-attr for "Hidden label"

;; Single-arity fn with hiccup body — must be reported.
(defn make-widget []
  (fn []
    [:div "Hidden in fn"]))

;; Named fn with arg and hiccup body.
(defn make-labeled [x]
  (fn label-fn [y]
    [:span {:title "Hidden label"} y]))

;; Multi-form body — only the hiccup form should be reported.
(defn multi-body []
  (fn []
    (let [x 1] x)
    [:p "Multi-body return"]))

;; Fn argument should still be a scope barrier for outer UI context.
;; "Enter" below is a DOM key name inside an event handler — must NOT be reported.
[:div {:on-key-down (fn [e] (when (= (.-key e) "Enter") nil))}
  "Outer visible text"]
