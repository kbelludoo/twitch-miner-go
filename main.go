package main

import (
	"context"
	"fmt"
	"os"
	"os/signal"
	"syscall"

	"github.com/kbelludoo/twitch-miner-go/pkg/config"
	"github.com/kbelludoo/twitch-miner-go/pkg/miner"
	"github.com/kbelludoo/twitch-miner-go/pkg/server"
)

func main() {
	fmt.Println("================================================================================")
	fmt.Println("     🟣 TWITCH CHANNEL POINTS MINER GO (ULTRA-LEVE & ALTA PERFORMANCE) 🟣")
	fmt.Println("             Zero Navegadores • Auto-Baú Instantâneo • ~2MB RAM")
	fmt.Println("================================================================================")

	configPath := "config.json"
	if _, err := os.Stat(configPath); os.IsNotExist(err) {
		configPath = "config.example.json"
	}

	cfg, err := config.LoadConfig(configPath)
	if err != nil {
		fmt.Printf("❌ Erro carregando configurações: %v\n", err)
		os.Exit(1)
	}

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, os.Interrupt, syscall.SIGTERM)
	go func() {
		<-sigChan
		fmt.Println("\nEncerrando Twitch Miner...")
		cancel()
	}()

	mgr := miner.NewManager(cfg)

	// Inicia Web Dashboard
	go func() {
		srv := server.NewServer(cfg.Settings.WebDashboardPort, mgr)
		if err := srv.Start(); err != nil {
			fmt.Printf("⚠ Erro no servidor web: %v\n", err)
		}
	}()

	mgr.Start(ctx)
}
