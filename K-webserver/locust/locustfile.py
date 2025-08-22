#!/usr/bin/env python3

import random
import time
from locust import HttpUser, task, between

class KWebappAPIUser(HttpUser):
    wait_time = between(1, 3)
    
    def on_start(self):
        """Initialize test data for API calls"""
        # Sample public keys for testing
        self.sample_pubkeys = [
            "02218b3732df2353978154ec5323b745bce9520a5ed506a96de4f4e3dad20dc44f",
            "033d01709a02bf78f95e09cd00ba93ad8fb7c8ac11e6d3f871a11062eeb2aa8cd8",
            "03f56f6ad1c1166e330fb2897ae60afcb25afa10006212cfee24264c04d21bce60",
            "03feff5d6e0d399a189ade44901c5f9b5bd959e6e2b82020446b78eae06732f21a",
            "02d22b4947724fa545681aa8281db260c1fbb081a6a53d8b4233c1be9f0bae4506",
            "028e6cb879cadeea0acef3fa34cd456ac5b12d533b6e42e8965804bc5506f0863e",
            "03b1f9ed63976b28ad4e648b9da672679bd27895def79159f372eeea30df45abf7",
            "028e6cb879cadeea0acef3fa34cd456ac5b12d533b6e42e8965804bc5506f0863e"
        ]
        
        # Sample post IDs for testing
        self.sample_post_ids = [
            "63e35ea479377d8cb36165f12dbaeb8b9c35a6b20e4c411254cfb910b570a48f",
            "ac0c73cf661259a3af663914e5d52f608a8430b47d9173164838d5198ae8d5d8",
            "baf60e2b8ef9027204ef8d1157b0908881a9deb5e8ad54d3de4ba9988f5f2fc3",
            "a44ca44bd09d54a6e6cc5919aca6e68cd7645498826e9c7bda92f824591d1495",
            "b3d26a66b57504ada5847bb635c639f3ecd734bc4deccaed0e64d75b8bae1c04",
            "794148d047ca5ef8729aea6aea0adda8d3eddbd593e01420d9930b4f9918d709",
            "f05e5f02ff1e27fd44a160801c5e4c81b8e198e8fcefaedac2e396451646e889",
            "9dc6942b6a3c880b49379b8bdbea1d8d68ec7bf8b6b3f941e9b86ffa2fd82d5e",
            "361013d8b5debb512b599615ad3739e8058d1b838ce71cec9bf1c0afadf32929",
            "fb44c8e5a5e14ee43c336e0d4ffe51bc64e883185fb83e5c1fe74dd305e6754a",
            "d79efc9b9aef2c93a030dbb53177fa5ab674cd29c1483335ff768524aad9e5cb",
            "78be2e16605ee6cd4f6903088d9e32936faa2655952a671475a940a2b7263fd3",
            "e710db11e2eff2a2ceeac00a085988278aaa646843c3582b6a73c18485477bb8",
            "602631308c97ba35f76d286ba69c34b580665cb83b5c5a35dc3f9f4c0a41b31c"
        ]
        
        # Default requester pubkey
        self.requester_pubkey = random.choice(self.sample_pubkeys)

    # @task(3)
    # def get_posts_following(self):
    #     """Test get-posts-following endpoint"""
    #     params = {
    #         'requesterPubkey': self.requester_pubkey,
    #         'limit': random.randint(5, 50)
    #     }
    #     
    #     # Add pagination parameters occasionally
    #     if random.random() < 0.3:
    #         params['before'] = int(time.time()) - random.randint(3600, 86400)
    #     elif random.random() < 0.1:
    #         params['after'] = int(time.time()) - random.randint(86400, 172800)
    #         
    #     with self.client.get("/get-posts-following", params=params, 
    #                        catch_response=True) as response:
    #         if response.status_code == 200:
    #             response.success()
    #         else:
    #             response.failure(f"Got {response.status_code}: {response.text}")

    @task(5)
    def get_posts_watching(self):
        """Test get-posts-watching endpoint"""
        params = {
            'requesterPubkey': self.requester_pubkey,
            'limit': random.randint(5, 50)
        }
        
        # Add pagination parameters occasionally
        if random.random() < 0.3:
            params['before'] = int(time.time()) - random.randint(3600, 86400)
        elif random.random() < 0.1:
            params['after'] = int(time.time()) - random.randint(86400, 172800)
            
        with self.client.get("/get-posts-watching", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(1)
    def get_mentions(self):
        """Test get-mentions endpoint"""
        params = {
            'user': random.choice(self.sample_pubkeys),
            'requesterPubkey': self.requester_pubkey,
            'limit': random.randint(5, 30)
        }
        
        # Add pagination parameters occasionally
        if random.random() < 0.2:
            params['before'] = int(time.time()) - random.randint(3600, 86400)
            
        with self.client.get("/get-mentions", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(2)
    def get_users(self):
        """Test get-users endpoint"""
        params = {
            'limit': random.randint(10, 100)
        }
        
        # Add pagination parameters occasionally
        if random.random() < 0.2:
            params['before'] = int(time.time()) - random.randint(3600, 86400)
            
        with self.client.get("/get-users", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(6)
    def get_posts(self):
        """Test get-posts endpoint (user posts)"""
        params = {
            'user': random.choice(self.sample_pubkeys),
            'requesterPubkey': self.requester_pubkey,
            'limit': random.randint(5, 50)
        }
        
        # Add pagination parameters occasionally
        if random.random() < 0.3:
            params['before'] = int(time.time()) - random.randint(3600, 86400)
        elif random.random() < 0.1:
            params['after'] = int(time.time()) - random.randint(86400, 172800)
            
        with self.client.get("/get-posts", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(4)
    def get_replies_by_post(self):
        """Test get-replies endpoint for specific post"""
        params = {
            'post': random.choice(self.sample_post_ids),
            'requesterPubkey': self.requester_pubkey,
            'limit': random.randint(5, 30)
        }
        
        # Add pagination parameters occasionally
        if random.random() < 0.2:
            params['before'] = int(time.time()) - random.randint(3600, 86400)
            
        with self.client.get("/get-replies", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(4)
    def get_replies_by_user(self):
        """Test get-replies endpoint for specific user"""
        params = {
            'user': random.choice(self.sample_pubkeys),
            'requesterPubkey': self.requester_pubkey,
            'limit': random.randint(5, 30)
        }
        
        # Add pagination parameters occasionally
        if random.random() < 0.2:
            params['before'] = int(time.time()) - random.randint(3600, 86400)
            
        with self.client.get("/get-replies", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(3)
    def get_post_details(self):
        """Test get-post-details endpoint"""
        params = {
            'id': random.choice(self.sample_post_ids),
            'requesterPubkey': self.requester_pubkey
        }
        
        with self.client.get("/get-post-details", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(1)
    def test_error_conditions(self):
        """Test various error conditions"""
        error_tests = [
            # Missing required parameters
            {"endpoint": "/get-posts", "params": {"user": self.sample_pubkeys[0]}},  # Missing requesterPubkey
            # {"endpoint": "/get-posts-following", "params": {"requesterPubkey": self.requester_pubkey}},  # Missing limit
            {"endpoint": "/get-mentions", "params": {"user": self.sample_pubkeys[0]}},  # Missing requesterPubkey
            
            # Invalid parameter values
            {"endpoint": "/get-posts", "params": {"user": self.sample_pubkeys[0], "requesterPubkey": self.requester_pubkey, "limit": 0}},  # limit too small
            {"endpoint": "/get-posts", "params": {"user": self.sample_pubkeys[0], "requesterPubkey": self.requester_pubkey, "limit": 150}},  # limit too large
            {"endpoint": "/get-post-details", "params": {"id": "invalid_post_id", "requesterPubkey": self.requester_pubkey}},  # Invalid post ID
        ]
        
        test = random.choice(error_tests)
        with self.client.get(test["endpoint"], params=test["params"],
                           catch_response=True) as response:
            if response.status_code in [400, 404]:
                response.success()  # Expected error responses
            elif response.status_code == 200:
                response.failure(f"Expected error but got 200: {response.text}")
            else:
                response.failure(f"Unexpected status {response.status_code}: {response.text}")


class KWebappHeavyLoadUser(HttpUser):
    """Heavy load testing user with more aggressive parameters"""
    wait_time = between(0.5, 1.5)
    weight = 1  # Lower weight, fewer of these users
    
    def on_start(self):
        """Initialize test data for heavy load testing"""
        self.sample_pubkeys = [
            "02218b3732df2353978154ec5323b745bce9520a5ed506a96de4f4e3dad20dc44f",
            "033d01709a02bf78f95e09cd00ba93ad8fb7c8ac11e6d3f871a11062eeb2aa8cd8",
            "03f56f6ad1c1166e330fb2897ae60afcb25afa10006212cfee24264c04d21bce60",
            "03feff5d6e0d399a189ade44901c5f9b5bd959e6e2b82020446b78eae06732f21a",
            "02d22b4947724fa545681aa8281db260c1fbb081a6a53d8b4233c1be9f0bae4506",
            "028e6cb879cadeea0acef3fa34cd456ac5b12d533b6e42e8965804bc5506f0863e",
            "03b1f9ed63976b28ad4e648b9da672679bd27895def79159f372eeea30df45abf7",
            "028e6cb879cadeea0acef3fa34cd456ac5b12d533b6e42e8965804bc5506f0863e"
        ]
        self.requester_pubkey = random.choice(self.sample_pubkeys)

    
    # @task(5)
    # def rapid_posts_following(self):
    #     """Rapid fire requests to get-posts-following"""
    #     params = {
    #         'requesterPubkey': self.requester_pubkey,
    #         'limit': 100  # Max limit for stress testing
    #     }
    #     
    #     with self.client.get("/get-posts-following", params=params,
    #                        catch_response=True) as response:
    #         if response.status_code == 200:
    #             response.success()
    #         else:
    #             response.failure(f"Got {response.status_code}: {response.text}")

    @task(3)
    def rapid_pagination_test(self):
        """Test rapid pagination requests"""
        base_params = {
            'requesterPubkey': self.requester_pubkey,
            'limit': 50
        }
        
        # Simulate pagination sequence
        timestamps = [
            int(time.time()) - 3600,    # 1 hour ago
            int(time.time()) - 7200,    # 2 hours ago  
            int(time.time()) - 10800,   # 3 hours ago
        ]
        
        for timestamp in timestamps:
            params = base_params.copy()
            params['before'] = timestamp
            
            with self.client.get("/get-posts-watching", params=params,
                               catch_response=True) as response:
                if response.status_code == 200:
                    response.success()
                else:
                    response.failure(f"Got {response.status_code}: {response.text}")


class KWebappRealisticUser(HttpUser):
    """Realistic user behavior simulation"""
    wait_time = between(2, 8)
    weight = 3  # Higher weight, more of these users
    
    def on_start(self):
        self.sample_pubkeys = [
            "02218b3732df2353978154ec5323b745bce9520a5ed506a96de4f4e3dad20dc44f",
            "033d01709a02bf78f95e09cd00ba93ad8fb7c8ac11e6d3f871a11062eeb2aa8cd8",
            "03f56f6ad1c1166e330fb2897ae60afcb25afa10006212cfee24264c04d21bce60",
            "03feff5d6e0d399a189ade44901c5f9b5bd959e6e2b82020446b78eae06732f21a",
            "02d22b4947724fa545681aa8281db260c1fbb081a6a53d8b4233c1be9f0bae4506",
            "028e6cb879cadeea0acef3fa34cd456ac5b12d533b6e42e8965804bc5506f0863e",
            "03b1f9ed63976b28ad4e648b9da672679bd27895def79159f372eeea30df45abf7",
            "028e6cb879cadeea0acef3fa34cd456ac5b12d533b6e42e8965804bc5506f0863e"
        ]
        self.requester_pubkey = random.choice(self.sample_pubkeys)
        self.session_posts = []

    # @task(10)
    # def browse_following_feed(self):
    #     """Simulate browsing following feed (most common action)"""
    #     params = {
    #         'requesterPubkey': self.requester_pubkey,
    #         'limit': random.choice([10, 20, 30])  # Typical page sizes
    #     }
    #     
    #     with self.client.get("/get-posts-following", params=params,
    #                        catch_response=True) as response:
    #         if response.status_code == 200:
    #             try:
    #                 data = response.json()
    #                 if 'posts' in data:
    #                     # Store some post IDs for later detail views
    #                     for post in data['posts'][:3]:  # Take first 3
    #                         if 'id' in post:
    #                             self.session_posts.append(post['id'])
    #                 response.success()
    #             except:
    #                 response.success()  # Still count as success even if JSON parsing fails
    #         else:
    #             response.failure(f"Got {response.status_code}: {response.text}")

    @task(2)
    def check_mentions(self):
        """Simulate checking mentions (common user behavior)"""
        params = {
            'user': self.requester_pubkey,
            'requesterPubkey': self.requester_pubkey,
            'limit': 20
        }
        
        with self.client.get("/get-mentions", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(3)
    def view_post_details(self):
        """Simulate clicking on a post to view details"""
        if self.session_posts:
            post_id = random.choice(self.session_posts)
        else:
            # Fallback to hardcoded post ID
            post_id = "63e35ea479377d8cb36165f12dbaeb8b9c35a6b20e4c411254cfb910b570a48f"
            
        params = {
            'id': post_id,
            'requesterPubkey': self.requester_pubkey
        }
        
        with self.client.get("/get-post-details", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(2)
    def browse_user_discovery(self):
        """Simulate browsing user introductions for discovery"""
        params = {
            'limit': random.choice([20, 50])
        }
        
        with self.client.get("/get-users", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(1)
    def check_own_posts(self):
        """Simulate checking own posts occasionally"""
        params = {
            'user': self.requester_pubkey,
            'requesterPubkey': self.requester_pubkey,
            'limit': 20
        }
        
        with self.client.get("/get-posts", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")


# Default user class (mix of all behaviors)
class WebsiteUser(KWebappAPIUser):
    weight = 2