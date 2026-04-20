(ns test.check-missing
  (:require [i18n :refer [t tt]]))

;; These keys ARE in the dictionary — should NOT appear as missing
(defn render-header []
  [:div (t :ui/save)
        (t :ui/cancel)])

(defn render-footer []
  (tt :nav/home))

;; These keys are NOT in the dictionary — should be detected as missing
(defn render-sidebar []
  (t :sidebar/title))

(defn render-dialog []
  (t :dialog/confirm-delete))

;; Keys in ignored namespace — should be filtered out
(defn render-legacy []
  (t :deprecated/extra-key))

;; Key matching always_used_key_patterns (shortcut.*) — should be filtered out
;; This simulates a dynamically composed key reference stored in a def
(defonce shortcut-keys
  [:shortcut/undefined-action])

(defn render-shortcut []
  (t shortcut-keys))
