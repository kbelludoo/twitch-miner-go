package twitch

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"
)

const (
	GQLURL   = "https://gql.twitch.tv/gql"
	ClientID = "kimne78kx3ncx6brgo4mv6wki5h1ko" // Official Twitch Web Client ID
)

type Client struct {
	httpCli *http.Client
}

func NewClient() *Client {
	return &Client{
		httpCli: &http.Client{
			Timeout: 15 * time.Second,
		},
	}
}

func (c *Client) postGQL(token string, query any, target any) error {
	payload, err := json.Marshal(query)
	if err != nil {
		return err
	}

	req, err := http.NewRequest("POST", GQLURL, bytes.NewReader(payload))
	if err != nil {
		return err
	}

	cleanToken := strings.TrimPrefix(token, "oauth:")
	req.Header.Set("Client-ID", ClientID)
	req.Header.Set("Authorization", "OAuth "+cleanToken)
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")

	resp, err := c.httpCli.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return err
	}

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("status HTTP %d: %s", resp.StatusCode, string(body))
	}

	return json.Unmarshal(body, target)
}

func (c *Client) GetCurrentUser(token string) (id, login, displayName string, err error) {
	reqBody := map[string]any{
		"query": `query { currentUser { id login displayName } }`,
	}
	var res CurrentUserResponse
	if err := c.postGQL(token, reqBody, &res); err != nil {
		return "", "", "", err
	}
	if res.Data.CurrentUser == nil {
		return "", "", "", fmt.Errorf("token inválido ou usuário não autenticado")
	}
	return res.Data.CurrentUser.ID, res.Data.CurrentUser.Login, res.Data.CurrentUser.DisplayName, nil
}

func (c *Client) GetChannelInfo(token, streamerLogin string) (*ChannelInfo, error) {
	reqBody := map[string]any{
		"query": `query($login: String!) {
			user(login: $login) {
				id
				login
				displayName
				stream {
					id
					title
					viewersCount
					type
				}
				channel {
					id
					self {
						communityPoints {
							balance
						}
					}
				}
			}
		}`,
		"variables": map[string]string{
			"login": strings.ToLower(streamerLogin),
		},
	}

	var res UserChannelResponse
	if err := c.postGQL(token, reqBody, &res); err != nil {
		return nil, err
	}

	u := res.Data.User
	if u == nil {
		return nil, fmt.Errorf("canal '%s' não encontrado", streamerLogin)
	}

	info := &ChannelInfo{
		ID:          u.ID,
		Login:       u.Login,
		DisplayName: u.DisplayName,
		IsLive:      u.Stream != nil && u.Stream.Type == "live",
	}

	if u.Stream != nil {
		info.StreamTitle = u.Stream.Title
		info.ViewerCount = u.Stream.ViewersCount
	}

	if u.Channel != nil && u.Channel.Self != nil && u.Channel.Self.CommunityPoints != nil {
		info.Points = u.Channel.Self.CommunityPoints.Balance
	}

	return info, nil
}

func (c *Client) ClaimBonusPoints(token, channelID, claimID string) (int, error) {
	reqBody := map[string]any{
		"query": `mutation($input: ClaimCommunityPointsInput!) {
			claimCommunityPoints(input: $input) {
				currentPoints
				pointsEarned
			}
		}`,
		"variables": map[string]any{
			"input": map[string]string{
				"channelID": channelID,
				"claimID":   claimID,
			},
		},
	}

	var res ClaimResponse
	if err := c.postGQL(token, reqBody, &res); err != nil {
		return 0, err
	}

	if len(res.Errors) > 0 {
		return 0, fmt.Errorf("erro resgatando bonus: %s", res.Errors[0].Message)
	}

	if res.Data.ClaimCommunityPoints == nil {
		return 0, fmt.Errorf("resposta vazia no resgate")
	}

	return res.Data.ClaimCommunityPoints.CurrentPoints, nil
}

func (c *Client) JoinRaid(token, raidID string) error {
	reqBody := map[string]any{
		"query": `mutation($input: JoinRaidInput!) {
			joinRaid(input: $input) {
				__typename
			}
		}`,
		"variables": map[string]any{
			"input": map[string]string{
				"raidID": raidID,
			},
		},
	}

	var res JoinRaidResponse
	if err := c.postGQL(token, reqBody, &res); err != nil {
		return err
	}

	if len(res.Errors) > 0 {
		return fmt.Errorf("%s", res.Errors[0].Message)
	}

	return nil
}
