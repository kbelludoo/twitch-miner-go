package server

import (
	_ "embed"
	"encoding/json"
	"fmt"
	"net/http"

	"github.com/kbelludoo/twitch-miner-go/pkg/miner"
)

//go:embed index.html
var htmlDashboard []byte

type Server struct {
	port    int
	manager *miner.Manager
}

func NewServer(port int, mgr *miner.Manager) *Server {
	return &Server{
		port:    port,
		manager: mgr,
	}
}

func (s *Server) Start() error {
	mux := http.NewServeMux()

	mux.HandleFunc("/api/status", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		data := map[string]any{
			"statuses": s.manager.GetStatuses(),
			"logs":     s.manager.GetLogs(),
		}
		_ = json.NewEncoder(w).Encode(data)
	})

	mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		_, _ = w.Write(htmlDashboard)
	})

	addr := fmt.Sprintf(":%d", s.port)
	fmt.Printf("[Web Server] Dashboard Twitch Miner online em http://localhost:%d\n", s.port)
	return http.ListenAndServe(addr, mux)
}
