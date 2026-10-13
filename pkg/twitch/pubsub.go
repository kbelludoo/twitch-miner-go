package twitch

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"net/http"
	"strings"
	"sync"
	"time"

	"github.com/gorilla/websocket"
)

const PubSubURL = "wss://pubsub-edge.twitch.tv/v1"

type PubSubManager struct {
	token       string
	userID      string
	client      *Client
	onClaimFunc func(channelID, claimID string)
	onRaidFunc  func(sourceChannelID string, raid RaidEventData)

	mu     sync.Mutex
	conn   *websocket.Conn
	topics map[string]bool
	stop   chan struct{}
}

func NewPubSubManager(token, userID string, client *Client, onClaim func(channelID, claimID string), onRaid func(sourceChannelID string, raid RaidEventData)) *PubSubManager {
	initialTopics := make(map[string]bool)
	initialTopics[fmt.Sprintf("community-points-user-v1.%s", userID)] = true

	return &PubSubManager{
		token:       token,
		userID:      userID,
		client:      client,
		onClaimFunc: onClaim,
		onRaidFunc:  onRaid,
		topics:      initialTopics,
		stop:        make(chan struct{}),
	}
}

func (p *PubSubManager) Start(ctx context.Context) {
	go p.loop(ctx)
}

func (p *PubSubManager) ListenTopic(topic string) {
	p.mu.Lock()
	defer p.mu.Unlock()

	if p.topics[topic] {
		return
	}
	p.topics[topic] = true

	if p.conn != nil {
		cleanToken := strings.TrimPrefix(p.token, "oauth:")
		listenMsg := map[string]any{
			"type":  "LISTEN",
			"nonce": randomNonce(),
			"data": map[string]any{
				"topics":     []string{topic},
				"auth_token": cleanToken,
			},
		}
		_ = p.conn.WriteJSON(listenMsg)
	}
}

func (p *PubSubManager) Close() {
	p.mu.Lock()
	defer p.mu.Unlock()
	select {
	case <-p.stop:
	default:
		close(p.stop)
	}
	if p.conn != nil {
		_ = p.conn.Close()
	}
}

func (p *PubSubManager) loop(ctx context.Context) {
	for {
		select {
		case <-ctx.Done():
			return
		case <-p.stop:
			return
		default:
		}

		err := p.connectAndListen(ctx)
		if err != nil {
			select {
			case <-ctx.Done():
				return
			case <-p.stop:
				return
			case <-time.After(5 * time.Second):
			}
		}
	}
}

func (p *PubSubManager) connectAndListen(ctx context.Context) error {
	dialer := websocket.DefaultDialer
	conn, _, err := dialer.DialContext(ctx, PubSubURL, http.Header{})
	if err != nil {
		return err
	}

	p.mu.Lock()
	p.conn = conn
	topicList := make([]string, 0, len(p.topics))
	for t := range p.topics {
		topicList = append(topicList, t)
	}
	p.mu.Unlock()

	defer func() {
		p.mu.Lock()
		if p.conn == conn {
			p.conn = nil
		}
		p.mu.Unlock()
		_ = conn.Close()
	}()

	cleanToken := strings.TrimPrefix(p.token, "oauth:")
	listenMsg := map[string]any{
		"type":  "LISTEN",
		"nonce": randomNonce(),
		"data": map[string]any{
			"topics":     topicList,
			"auth_token": cleanToken,
		},
	}

	if err := conn.WriteJSON(listenMsg); err != nil {
		return err
	}

	// Ping a cada 4 minutos (Twitch exige < 5 min)
	pingTicker := time.NewTicker(4 * time.Minute)
	defer pingTicker.Stop()

	go func() {
		for {
			select {
			case <-ctx.Done():
				return
			case <-p.stop:
				return
			case <-pingTicker.C:
				p.mu.Lock()
				if p.conn != nil {
					_ = p.conn.WriteJSON(map[string]string{"type": "PING"})
				}
				p.mu.Unlock()
			}
		}
	}()

	for {
		_ = conn.SetReadDeadline(time.Now().Add(5 * time.Minute))
		_, msg, err := conn.ReadMessage()
		if err != nil {
			return err
		}

		var event struct {
			Type string `json:"type"`
			Data struct {
				Topic   string `json:"topic"`
				Message string `json:"message"`
			} `json:"data"`
		}

		if err := json.Unmarshal(msg, &event); err != nil {
			continue
		}

		if event.Type == "MESSAGE" {
			// Eventos de Pontos de Canal / Baús
			if strings.HasPrefix(event.Data.Topic, "community-points-user-v1") {
				var pointPayload struct {
					Type string `json:"type"`
					Data struct {
						Claim struct {
							ID        string `json:"id"`
							ChannelID string `json:"channel_id"`
						} `json:"claim"`
					} `json:"data"`
				}
				if err := json.Unmarshal([]byte(event.Data.Message), &pointPayload); err == nil {
					if pointPayload.Type == "claim-available" && pointPayload.Data.Claim.ID != "" {
						if p.onClaimFunc != nil {
							go p.onClaimFunc(pointPayload.Data.Claim.ChannelID, pointPayload.Data.Claim.ID)
						}
					}
				}
			}

			// Eventos de Raids
			if strings.HasPrefix(event.Data.Topic, "raid.") {
				sourceChannelID := strings.TrimPrefix(event.Data.Topic, "raid.")
				var raidPayload struct {
					Type string `json:"type"`
					Raid struct {
						ID                string `json:"id"`
						SourceID          string `json:"source_id"`
						TargetID          string `json:"target_id"`
						TargetLogin       string `json:"target_login"`
						TargetDisplayName string `json:"target_display_name"`
					} `json:"raid"`
				}
				if err := json.Unmarshal([]byte(event.Data.Message), &raidPayload); err == nil {
					if (raidPayload.Type == "raid_update_v2" || raidPayload.Type == "raid_start") && raidPayload.Raid.ID != "" {
						if p.onRaidFunc != nil {
							go p.onRaidFunc(sourceChannelID, RaidEventData{
								ID:                raidPayload.Raid.ID,
								SourceID:          raidPayload.Raid.SourceID,
								TargetID:          raidPayload.Raid.TargetID,
								TargetLogin:       raidPayload.Raid.TargetLogin,
								TargetDisplayName: raidPayload.Raid.TargetDisplayName,
							})
						}
					}
				}
			}
		}
	}
}

func randomNonce() string {
	b := make([]byte, 8)
	_, _ = rand.Read(b)
	return hex.EncodeToString(b)
}
