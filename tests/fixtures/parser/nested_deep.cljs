;; Deep nesting test (10+ levels)

(defn deeply-nested []
  [:div
    [:section
      [:article
        [:div
          [:div
            [:div
              [:div
                [:div
                  [:div
                    [:div
                      [:span "Deep text"]]]]]]]]]]])

;; Deeply nested function calls
(let [x (when (if (cond (= a b) (do (let [y 1] (fn [] (str "deep" y))))))])
