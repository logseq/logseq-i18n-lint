;; Basic S-expression parsing test fixtures

;; Simple list
(defn hello [] (println "world"))

;; Vector
[1 2 3]

;; Map
{:name "Alice" :age 30}

;; Set
#{:a :b :c}

;; Nested structure
(defn render []
  [:div {:class "container"}
    [:h1 "Hello"]
    [:ul
      [:li "Item 1"]
      [:li "Item 2"]]])

;; Keywords with namespaces
:user/name
::auto-resolved

;; Numbers
42
3.14
-1
0xFF
1/3

;; Characters
\a
\newline
\space
\tab

;; Boolean and nil
true
false
nil
