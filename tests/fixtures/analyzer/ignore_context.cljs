;; Ignore context test - these should NOT be reported

;; Console logging
(js/console.log "Debug: component mounted")
(js/console.error "Error: failed to load")
(js/console.warn "Warning: deprecated API")

;; Logging functions
(log/debug "Processing block" block-id)
(log/info "Server started on port" port)
(log/warn "Config missing" key)
(log/error "Unhandled exception" ex)

;; Print functions
(prn "debug value" value)
(println "Status:" status)

;; Regex operations
(re-pattern "^[a-z]+$")
(re-find "pattern" input)
(re-matches "^\\d+$" s)

;; Namespace declarations
(ns myapp.core
  (:require [clojure.string :as str]
            [myapp.util :as util]))

(require '[clojure.test :refer [deftest is]])
