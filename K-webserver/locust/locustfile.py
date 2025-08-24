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
            "ba13aef5f48d0c50b8413b3e97a9b96b7934b4ef172b52bbcb5d3629054fdae1",
            "789df90a23784221db8430ce621c1b92b8f819661a5869b9c4337897699b9aa8",
            "8ff32e368f53eb795f0612f9ff7fcf56aab438893fc7ace80ea2ff4a26787e69",
            "3e5f09dfc177a0295ffd871bfc7c17a07a40a08b08b5d741da3e582a349a8a68",
            "473e732c996bb682a42a31a03ee310e9c7eb0221cfcc973cb0666bd83c9e19f8",
            "6a9176d00eb87faa617702a6b779d500618cbc614e337935cf1094bd1a5dcf82",
            "2d6a49d15975909c12526b0b0dfda44b491439e61caef567f90f714ab4f98e39",
            "2ff8738d44b000b79e4bdacc029d77a399012e705ddeb0bb7385c790e01bb702",
            "1306a7f26d7ad994654a2f50e66418e02059785d608e96c02e0e44d217deae20",
            "88ceeae0ded17c1687309597d0629227c71d2fa78843541e53efa41dad7fecad",
            "c93bead84c1317523d3fa78907cf6d0d1615a7769031d26e883389223ebc06e2",
            "ec2e9464aee5209d8de72f36a2cf3587e9c34c4241b64480532e73ae0a12e873",
            "385aa34cdb40969d83842bdb55fc5ebafff6b1b6152852e94bea15440b16571d",
            "ddede12334f78a1e808a8e6bdb16a95ecee12f539e2476ac483561b07b3f5cd9",
            "6e9b79aea09d085a88736174beed12c42d16cf129d5d2433926391f86234c472",
            "6d59b1e1318010d690ca9b0914b969f3ad9151ca7f9959216a48ffc38a91460d",
            "55774ef271e906b36cbd84d438dd60bd5dc241c44efb06d40dd84b0337b83246",
            "e5bda626da2a90b9326f8e23b6af9dc4af20237f10ee4852302feee13b30785e",
            "f44cc1fb19eac3c7661bf8c7c25ad118675acc26cde211d7eae8d0a17724c813",
            "448dc02b7df228af266a95ca2b431603bd90baea76af4fb59dd977ed3d429c91"
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
            post_id = "6a9176d00eb87faa617702a6b779d500618cbc614e337935cf1094bd1a5dcf82"
            
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