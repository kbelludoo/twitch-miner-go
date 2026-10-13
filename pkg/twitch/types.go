package twitch

type CurrentUserResponse struct {
	Data struct {
		CurrentUser *struct {
			ID          string `json:"id"`
			Login       string `json:"login"`
			DisplayName string `json:"displayName"`
		} `json:"currentUser"`
	} `json:"data"`
}

type ChannelInfo struct {
	ID          string
	Login       string
	DisplayName string
	IsLive      bool
	StreamTitle string
	ViewerCount int
	Points      int
	ClaimID     string
}

type UserChannelResponse struct {
	Data struct {
		User *struct {
			ID          string `json:"id"`
			Login       string `json:"login"`
			DisplayName string `json:"displayName"`
			Stream      *struct {
				ID           string `json:"id"`
				Title        string `json:"title"`
				ViewersCount int    `json:"viewersCount"`
				Type         string `json:"type"`
			} `json:"stream"`
			Channel *struct {
				ID   string `json:"id"`
				Self *struct {
					CommunityPoints *struct {
						Balance int `json:"balance"`
					} `json:"communityPoints"`
				} `json:"self"`
			} `json:"channel"`
		} `json:"user"`
	} `json:"data"`
}

type ClaimResponse struct {
	Data struct {
		ClaimCommunityPoints *struct {
			CurrentPoints int `json:"currentPoints"`
			PointsEarned  int `json:"pointsEarned"`
		} `json:"claimCommunityPoints"`
	} `json:"data"`
	Errors []struct {
		Message string `json:"message"`
	} `json:"errors"`
}

type RaidEventData struct {
	ID                string `json:"id"`
	SourceID          string `json:"source_id"`
	TargetID          string `json:"target_id"`
	TargetLogin       string `json:"target_login"`
	TargetDisplayName string `json:"target_display_name"`
}

type JoinRaidResponse struct {
	Data struct {
		JoinRaid *struct {
			Typename string `json:"__typename"`
		} `json:"joinRaid"`
	} `json:"data"`
	Errors []struct {
		Message string `json:"message"`
	} `json:"errors"`
}
