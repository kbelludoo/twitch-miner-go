package miner

import (
	"context"
	"fmt"
	"runtime"
	"strings"
	"sync"
	"time"

	"github.com/kbelludoo/twitch-miner-go/pkg/config"
	"github.com/kbelludoo/twitch-miner-go/pkg/twitch"
)

type StreamerStatus struct {
	Login       string `json:"login"`
	DisplayName string `json:"displayName"`
	IsLive      bool   `json:"isLive"`
	Title       string `json:"title"`
	Viewers     int    `json:"viewers"`
	Points      int    `json:"points"`
	Watching    bool   `json:"watching"`
	LastClaimAt string `json:"lastClaimAt"`
}

type AccountStatus struct {
	Username  string                    `json:"username"`
	UserID    string                    `json:"userId"`
	Streamers map[string]StreamerStatus `json:"streamers"`
}

type Manager struct {
	cfg       *config.Config
	client    *twitch.Client
	startedAt time.Time

	mu        sync.RWMutex
	statuses  map[string]*AccountStatus
	pubsubs   map[string]*twitch.PubSubManager
	activeMap map[string]map[string]bool // username -> streamer -> watching

	logsMu    sync.RWMutex
	logs      []string
}

func NewManager(cfg *config.Config) *Manager {
	return &Manager{
		cfg:       cfg,
		client:    twitch.NewClient(),
		startedAt: time.Now(),
		statuses:  make(map[string]*AccountStatus),
		pubsubs:   make(map[string]*twitch.PubSubManager),
		activeMap: make(map[string]map[string]bool),
		logs:      make([]string, 0, 100),
	}
}

func (m *Manager) AddLog(msg string) {
	m.logsMu.Lock()
	defer m.logsMu.Unlock()
	entry := fmt.Sprintf("[%s] %s", time.Now().Format("15:04:05"), msg)
	m.logs = append(m.logs, entry)
	if len(m.logs) > 200 {
		m.logs = m.logs[len(m.logs)-200:]
	}
	fmt.Println(entry)
}

func (m *Manager) GetLogs() []string {
	m.logsMu.RLock()
	defer m.logsMu.RUnlock()
	copied := make([]string, len(m.logs))
	copy(copied, m.logs)
	return copied
}

func (m *Manager) GetStatuses() map[string]AccountStatus {
	m.mu.RLock()
	defer m.mu.RUnlock()
	res := make(map[string]AccountStatus)
	for u, st := range m.statuses {
		copySt := AccountStatus{
			Username:  st.Username,
			UserID:    st.UserID,
			Streamers: make(map[string]StreamerStatus),
		}
		for s, info := range st.Streamers {
			copySt.Streamers[s] = info
		}
		res[u] = copySt
	}
	return res
}

func (m *Manager) Start(ctx context.Context) {
	m.AddLog(fmt.Sprintf("Iniciando Twitch Miner Go para %d conta(s)...", len(m.cfg.Accounts)))

	for _, acc := range m.cfg.Accounts {
		uID, login, _, err := m.client.GetCurrentUser(acc.AuthToken)
		if err != nil {
			m.AddLog(fmt.Sprintf("[%s] ❌ Erro autenticando conta: %v", acc.Username, err))
			continue
		}

		m.mu.Lock()
		st := &AccountStatus{
			Username:  acc.Username,
			UserID:    uID,
			Streamers: make(map[string]StreamerStatus),
		}
		for _, s := range acc.Streamers {
			clean := strings.ToLower(strings.TrimSpace(s))
			if clean != "" {
				st.Streamers[clean] = StreamerStatus{Login: clean}
			}
		}
		m.statuses[acc.Username] = st
		m.activeMap[acc.Username] = make(map[string]bool)
		m.mu.Unlock()

		m.AddLog(fmt.Sprintf("[%s] ✅ Autenticado com sucesso na Twitch! (ID: %s, Login: %s)", acc.Username, uID, login))

		// Inicia PubSub para escuta de bônus e raids em tempo real
		ps := twitch.NewPubSubManager(acc.AuthToken, uID, m.client, func(channelID, claimID string) {
			m.handleClaimBonus(acc, channelID, claimID)
		}, func(sourceChannelID string, raid twitch.RaidEventData) {
			m.handleRaid(acc, sourceChannelID, raid)
		})
		ps.Start(ctx)

		m.mu.Lock()
		m.pubsubs[acc.Username] = ps
		m.mu.Unlock()
	}

	// Executa checagem inicial
	m.checkAll(ctx)

	// Inicia ticker de monitoramento no console
	go m.tickerLoop(ctx)

	// Loop periódico de checagem de lives
	checkTicker := time.NewTicker(time.Duration(m.cfg.Settings.CheckIntervalSeconds) * time.Second)
	defer checkTicker.Stop()

	for {
		select {
		case <-ctx.Done():
			m.Close()
			return
		case <-checkTicker.C:
			m.checkAll(ctx)
		}
	}
}

func (m *Manager) Close() {
	m.mu.Lock()
	defer m.mu.Unlock()
	for _, ps := range m.pubsubs {
		ps.Close()
	}
}

func (m *Manager) checkAll(ctx context.Context) {
	for _, acc := range m.cfg.Accounts {
		go m.checkAccount(ctx, acc)
	}
}

func (m *Manager) checkAccount(ctx context.Context, acc config.AccountConfig) {
	maxSlots := acc.MaxConcurrent
	if maxSlots <= 0 {
		maxSlots = 5
	}

	activeCount := 0
	m.mu.RLock()
	for _, watching := range m.activeMap[acc.Username] {
		if watching {
			activeCount++
		}
	}
	m.mu.RUnlock()

	for _, streamer := range acc.Streamers {
		clean := strings.ToLower(strings.TrimSpace(streamer))
		if clean == "" {
			continue
		}

		info, err := m.client.GetChannelInfo(acc.AuthToken, clean)
		if err != nil {
			continue
		}

		m.mu.Lock()
		st := m.statuses[acc.Username]
		status := st.Streamers[clean]
		status.DisplayName = info.DisplayName
		status.IsLive = info.IsLive
		status.Title = info.StreamTitle
		status.Viewers = info.ViewerCount
		status.Points = info.Points

		wasWatching := m.activeMap[acc.Username][clean]

		if info.IsLive {
			if !wasWatching && activeCount < maxSlots {
				m.activeMap[acc.Username][clean] = true
				status.Watching = true
				activeCount++
				m.AddLog(fmt.Sprintf("[%s] ⭐ Assistindo live de '%s'! Pontos: %d", acc.Username, info.DisplayName, info.Points))
			} else if wasWatching {
				status.Watching = true
			}
		} else {
			if wasWatching {
				m.activeMap[acc.Username][clean] = false
				status.Watching = false
				activeCount--
				m.AddLog(fmt.Sprintf("[%s] 📴 Live de '%s' finalizou; slot liberado.", acc.Username, info.DisplayName))
			}
		}

		// Se houver baú disponível retornado na query, resgata
		if info.ClaimID != "" && m.cfg.Settings.AutoClaimBonus {
			go m.handleClaimBonus(acc, info.ID, info.ClaimID)
		}

		// Se estiver assistindo ao canal, escuta eventos de raid via PubSub
		if status.Watching && info.ID != "" {
			if ps, ok := m.pubsubs[acc.Username]; ok && ps != nil {
				ps.ListenTopic("raid." + info.ID)
			}
		}

		st.Streamers[clean] = status
		m.mu.Unlock()
	}
}

func (m *Manager) handleRaid(acc config.AccountConfig, sourceChannelID string, raid twitch.RaidEventData) {
	m.AddLog(fmt.Sprintf("[%s] 🚀 RAID DETECTADA! O streamer iniciou raid para '%s' (ID: %s)! Participando (+250 pts)...",
		acc.Username, raid.TargetDisplayName, raid.ID))

	if m.cfg.Settings.AutoJoinRaid {
		err := m.client.JoinRaid(acc.AuthToken, raid.ID)
		if err != nil {
			m.AddLog(fmt.Sprintf("[%s] ℹ Presença na raid registrada via WebSocket (%v)", acc.Username, err))
		} else {
			m.AddLog(fmt.Sprintf("[%s] ✅ Confirmação de JoinRaid enviada com sucesso (+250 pontos)!", acc.Username))
		}
	}

	if m.cfg.Settings.FollowRaids && raid.TargetLogin != "" {
		target := strings.ToLower(raid.TargetLogin)
		m.AddLog(fmt.Sprintf("[%s] 🔀 Seguindo raid para o canal '%s' para manter o farm 24/7!", acc.Username, raid.TargetDisplayName))

		m.mu.Lock()
		st := m.statuses[acc.Username]
		if st != nil {
			if _, exists := st.Streamers[target]; !exists {
				st.Streamers[target] = StreamerStatus{
					Login:       target,
					DisplayName: raid.TargetDisplayName,
					IsLive:      true,
					Watching:    true,
				}
			}
			m.activeMap[acc.Username][target] = true
		}
		m.mu.Unlock()

		go m.checkAccount(context.Background(), acc)
	}
}

func (m *Manager) handleClaimBonus(acc config.AccountConfig, channelID, claimID string) {
	newPoints, err := m.client.ClaimBonusPoints(acc.AuthToken, channelID, claimID)
	if err != nil {
		return
	}
	m.AddLog(fmt.Sprintf("[%s] 🎁 Baú de bônus resgatado (+50 pts)! Novo saldo: %d pontos.", acc.Username, newPoints))
}

func (m *Manager) tickerLoop(ctx context.Context) {
	ticker := time.NewTicker(5 * time.Second)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
			var mem runtime.MemStats
			runtime.ReadMemStats(&mem)
			ramMB := float64(mem.Alloc) / 1024 / 1024
			uptime := time.Since(m.startedAt).Round(time.Second)

			m.mu.RLock()
			activeList := make([]string, 0)
			totalPoints := 0
			for u, actMap := range m.activeMap {
				st := m.statuses[u]
				for s, watching := range actMap {
					if watching {
						pts := 0
						if st != nil {
							pts = st.Streamers[s].Points
						}
						totalPoints += pts
						activeList = append(activeList, fmt.Sprintf("%s (%d pts)", s, pts))
					}
				}
			}
			m.mu.RUnlock()

			fmt.Printf("[Ticker] RAM: %.2f MB | Uptime: %s | Canais: %d | Total Pontos: %d | Assistindo: %v\n",
				ramMB, uptime, len(activeList), totalPoints, activeList)
		}
	}
}
