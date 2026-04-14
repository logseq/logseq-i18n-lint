(ns test.app
  (:require [i18n :refer [t tt]]))

;; Direct translation calls
(defn render-header []
  [:div (t :ui/save)
        (t :ui/cancel)])

;; Aliased call
(defn render-footer []
  (tt :nav/home))

;; Conditional translation
(defn loading-indicator [loading?]
  [:span (t (if loading? :ui/loading :ui/ready))])

;; Map with :i18n-key
(def dialog-config
  {:i18n-key :dialog/confirm
   :title-key :dialog/title
   :prompt-key :dialog/prompt})

;; Conditional in :i18n-key
(defn theme-label [dark?]
  {:i18n-key (if dark? :theme/dark :theme/light)})

;; cond translation
(defn message-for-type [type]
  (t (cond
       (= type :a) :msg/type-a
       (= type :b) :msg/type-b
       :else :msg/default)))

;; def with keyword vector + symbol resolution
(defonce view-options
  [[:view/option-a :asc]
   [:view/option-b :desc]])

(defn render-options []
  (for [[label _] view-options]
    (t view-options)))
