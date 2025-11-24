// Simple Go backend server for load testing
// Minimal latency, configurable response delays
package main

import (
	"flag"
	"fmt"
	"log"
	"net/http"
	"runtime"
	"sync/atomic"
	"time"
)

var (
	port     = flag.Int("port", 8081, "Port to listen on")
	delay    = flag.Duration("delay", 0, "Response delay (e.g., 10ms)")
	reqCount uint64
)

func main() {
	flag.Parse()

	// Use all CPU cores
	runtime.GOMAXPROCS(runtime.NumCPU())

	// Health check endpoint
	http.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/plain")
		w.WriteHeader(http.StatusOK)
		fmt.Fprintf(w, "healthy\n")
	})

	// Main endpoint with optional delay
	http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		atomic.AddUint64(&reqCount, 1)

		if *delay > 0 {
			time.Sleep(*delay)
		}

		w.Header().Set("Content-Type", "text/plain")
		w.Header().Set("X-Backend-Server", "go-backend")
		w.WriteHeader(http.StatusOK)
		fmt.Fprintf(w, "OK\n")
	})

	// Slow endpoint (fixed 100ms delay)
	http.HandleFunc("/slow", func(w http.ResponseWriter, r *http.Request) {
		atomic.AddUint64(&reqCount, 1)
		time.Sleep(100 * time.Millisecond)
		w.Header().Set("Content-Type", "text/plain")
		w.WriteHeader(http.StatusOK)
		fmt.Fprintf(w, "slow response\n")
	})

	// Large payload endpoint
	http.HandleFunc("/large", func(w http.ResponseWriter, r *http.Request) {
		atomic.AddUint64(&reqCount, 1)
		w.Header().Set("Content-Type", "text/plain")
		w.WriteHeader(http.StatusOK)
		// 10KB response
		data := make([]byte, 10240)
		for i := range data {
			data[i] = 'A' + byte(i%26)
		}
		w.Write(data)
	})

	// Stats endpoint
	http.HandleFunc("/stats", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/plain")
		count := atomic.LoadUint64(&reqCount)
		fmt.Fprintf(w, "Total requests: %d\n", count)
	})

	// Start stats logger
	go func() {
		ticker := time.NewTicker(10 * time.Second)
		lastCount := uint64(0)
		for range ticker.C {
			current := atomic.LoadUint64(&reqCount)
			delta := current - lastCount
			rps := float64(delta) / 10.0
			log.Printf("Requests/sec: %.0f, Total: %d", rps, current)
			lastCount = current
		}
	}()

	addr := fmt.Sprintf(":%d", *port)
	log.Printf("Starting backend server on %s", addr)
	log.Printf("Response delay: %v", *delay)
	log.Fatal(http.ListenAndServe(addr, nil))
}
