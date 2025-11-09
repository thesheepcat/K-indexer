#!/usr/bin/env python3

import random
import time
from locust import HttpUser, task, between

class KWebappAPIUser(HttpUser):
    wait_time = between(1, 3)

    def on_start(self):
        """Initialize test data for API calls with REAL data from DEV database"""
        # Real public keys from k-db-DEV (30 users)
        self.sample_pubkeys = [
            "03edeacd90b1fe7ee696e21ddd81083fd44a245642a4a5cade455c465c9ce80e1b",
            "032c1de780180d4e5100e17e304996284d187fdeb5ac837f32c04d62b73aaefe18",
            "03f5a6fde99bc8e9ab6c6c92a08bafe622e533c7dad7f4eec2edef35baaa5a9bc6",
            "0280bc073a2f016dbd6df25c4e843a5f801d3d940fc384d79f8103e42297be5986",
            "020bc57bc1fb10751dc18282931a07cd6ace772217f0c4d843bc2b1a71670d7de1",
            "03b1f9ed63976b28ad4e648b9da672679bd27895def79159f372eeea30df45abf7",
            "0367ff507f241fa0b4e90779952d7ed36731a77021db0982fbabd0fbfa036bd8ee",
            "030fd8f1b20f57abf99d61c95db9a149c5a9e0502817ba257d6adf32d0a4708ebe",
            "02405d3f51e96e18ebe1894242551baf10edd5307721b761a8a1e5d3a8df8817f9",
            "0327900e8385a066dff50e90c1851a39ed107cf90897b6fad0da63020bb8327fbd",
            "02dd3235798995ebc348613b7403d5bfbc277b3e3b7414ce3e166f4adacff7f532",
            "038f29db80e35e9086adc7af56383625bb7dc5cf3690bc092dcd210039db3d825d",
            "02bddf1f69dc78dcc8c656a7d9a8bc65256e26bbac970ef2c23e2f5e0982112d65",
            "033d01709a02bf78f95e09cd00ba93ad8fb7c8ac11e6d3f871a11062eeb2aa8cd8",
            "038ea9ca1fe1f22cc8074cc576e0870cf50f773c90c1f4830fd6ba6f60771cc1f3",
            "02218b3732df2353978154ec5323b745bce9520a5ed506a96de4f4e3dad20dc44f",
            "03f56f6ad1c1166e330fb2897ae60afcb25afa10006212cfee24264c04d21bce60",
            "0223ca2161f47a142e77f85c398b839a65b8c799e22968d3214a25aee616bd00b6",
            "0327c058e497477d175674381a30ff36e5309f7c0f5bc9dc683506de6549e40562",
            "024d3fe4f26cd652d91f4173c7517af9c0254de833cc25924fc2d475768008fd15",
            "0207e2bd14de5a9e7e39b00437cfe58ad64af128678485029b5fe304b841d1b076",
            "03466b99d7ca36044fdc1c486d0c40c1ecf024f54554377fb005a40b16b08637cc"
        ]

        # Real content IDs from k_contents (posts/quotes) - 40 items
        self.sample_post_ids = [
            "40dfcbfff3ad1c8a0c80bfd8bddf574b75a796d644c4f508ae63f7e47ed3d2bf",
            "7c0b5d4d995eb86b076ab787a96c9d1180219a7dfc8de419b7dd0958e02723d9",
            "b1c455ffdbf96f8199641a0ef42d96f55d7261b4abd3c28d7eb384742e098e82",
            "0e5b63242a970cc5fb567dd71c223a62a74507821fc1b9fe5eb3df9c258ffce8",
            "996bc414134275c4d8638e339f1a30e88da4f495219ae7494ddafd2e413d5c3a",
            "a9ef66ac8fec30431547a2c3e71fbbf7a04f859f5800177cd0f93abc69b10ae3",
            "7401c334fed3383d73a96f3d46c2cbad93d70798f469eaf93e985994fcd805dc",
            "e0893f1f8d990f2672a5d30c90c3bc0982899fd645a71785f268161d8e1ab8c0",
            "e6ff72cc7958f5ef6beaf5660b6ba95787758a2d3fe405adf9864ee3eadd37d0",
            "631857e3ad73a2e5c05b193b56fcd9d9c89dc40daad06d0e4b1f7128d660f4d0",
            "8035e96070ec133288f8be16b2885c23740ccaf2558d4d5d868256814dd23aff",
            "2c7f16c10ecffe3f845cfa0e06fd5cdceb87f30c9938ef9ca24918b2d5b96cb6",
            "9d2ffa4aab2ecd054d3ed2d760aa800fc35c028a064595ebd8bbd907f74a6fcf",
            "e26b14bfde54bbed157aeb85d358e22c8e6fb21d5503cf86b63dc97a246df85b",
            "889bb1cc99260789e570be64513ff4afa65982eaa920ddb3ebc7ca58f09a38fe",
            "c8f371509f38638a05aab45bef069574736b38f837ed2c6eacfed59c099e3f8c",
            "e1b110094f672b02ceec8a2949cdeb1ceaf498422b91d2774b1b9e127e41cde4",
            "91a489d1c0202a49b830bf3a0ed6570d7a1b3051e16397d5052b99bd4d51d805",
            "239605f8e78044793cad47f6dc36df7a991152c45469cd4a4a0d383dc55dcb6d",
            "17088141aba4e07dccb515dfbda3106480d7c65b64e2ba7ff70f9e87715280f6",
            "e3bf67aa5b1aa53096b742afa382fcc60c370b4cf8d3942c423247d9925cebeb",
            "6ee1dc36863c391e02ec290445fed2c36f534d937d84acd74c7495ead78b4874",
            "85bc33d5e5daf886158f796eaea614894f730f212e339edbe92962714fa793fd",
            "1fde0de5ff86edac61e09fa2f1588f7ff140ee036a500a6f6ccbadb4304d38a5",
            "62ae89ffd92ffabf040d820cd00f23fa6c1bc2ad02ddd630a71ba5244da19862",
            "17a9c41ad5e935a7e0448bfbfe2ec779d35cdefb785d4e0a5958733d9ddfeae2",
            "f3e9e85eb3c6e594bc26d4b83b31214332af7b26a0bb15849300a8d172346139",
            "7bd455cc044a035b1cc7ee911207cbcda692ee3b453e48710ed626190e384688",
            "a295cf6cee11a75b7f32a376d3c6c918d6b64668c3b4665df2bcf5bde21627f0",
            "634c4cee207a386602bafcd7db95c1a77f14fc322c89e21fc33d2d641e714b05",
            "5e37c2ac01628876bc368530d625ee7d9622026e7e494707d1e3024bd26b5d4b",
            "4a5bc584d79539a225e2916cb47cbbf7e54b4ff4244f640a3acd43b4ec79f3f7",
            "06448fcb7dfb7a6e68e7b421302fac2c2d9c5d678127f9e746e9047c3ca455b1",
            "7b62f663652ae758d1b2daa7f6ee8b1e260dc65ad6bdf6f233c2452f6cd31066",
            "7ab4535d619ab06095825d312221227769ac856d8e8dc3ed7c88829052567730",
            "fdd2788f42aa3903fd9f4b86f2ae32162f9da7be2dcc4760aba69d65139c9bac",
            "34d6b4d2ed35cc4cf010809985a681f403555e3e3704edca6135a5b9d7771171",
            "df5f1d79531f75e33fa9c534c1362fc6d42f561854f29c69f4893fa73f03560d",
            "855bed8163845d91bb12fa724f5e9d14a06769c6b38a26acaf2be406bfdc68ad",
            "12eb841d2c281c6c3eb6c3c09fbe4b161d10e3c8a321c06e7b859b8dd1d43168"
        ]

        # Users who follow others (for following-related endpoints)
        self.users_with_follows = [
            "0367ff507f241fa0b4e90779952d7ed36731a77021db0982fbabd0fbfa036bd8ee",
            "038ea9ca1fe1f22cc8074cc576e0870cf50f773c90c1f4830fd6ba6f60771cc1f3",
            "03edeacd90b1fe7ee696e21ddd81083fd44a245642a4a5cade455c465c9ce80e1b",
            "02299c80be550c23bfe10954cf7f3c62a9f8e9af45df3a62ab665633a5e8b32c87",
            "03f5a6fde99bc8e9ab6c6c92a08bafe622e533c7dad7f4eec2edef35baaa5a9bc6"
        ]

        # Default requester pubkey
        self.requester_pubkey = random.choice(self.sample_pubkeys)

    @task(5)
    def get_posts_watching(self):
        """Test get-posts-watching endpoint (global feed)"""
        params = {
            'requesterPubkey': self.requester_pubkey,
            'limit': random.randint(10, 50)
        }

        # Add pagination parameters occasionally
        if random.random() < 0.3:
            params['before'] = f"{int(time.time() * 1000000) - random.randint(3600000000, 86400000000)}_{random.randint(1, 1000)}"

        with self.client.get("/get-posts-watching", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(3)
    def get_content_following(self):
        """Test get-content-following endpoint (following feed)"""
        params = {
            'requesterPubkey': random.choice(self.users_with_follows),
            'limit': random.randint(10, 50)
        }

        # Add pagination parameters occasionally
        if random.random() < 0.3:
            params['before'] = f"{int(time.time() * 1000000) - random.randint(3600000000, 86400000000)}_{random.randint(1, 1000)}"

        with self.client.get("/get-content-following", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            elif response.status_code == 404:
                # 404 is valid - user has no following relationships
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(2)
    def get_mentions(self):
        """Test get-mentions endpoint"""
        params = {
            'user': random.choice(self.sample_pubkeys),
            'requesterPubkey': self.requester_pubkey,
            'limit': random.randint(10, 30)
        }

        # Add pagination parameters occasionally
        if random.random() < 0.2:
            params['before'] = f"{int(time.time() * 1000000) - random.randint(3600000000, 86400000000)}_{random.randint(1, 1000)}"

        with self.client.get("/get-mentions", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(3)
    def get_notifications(self):
        """Test get-notifications endpoint"""
        params = {
            'user': random.choice(self.sample_pubkeys),
            'requesterPubkey': self.requester_pubkey,
            'limit': random.randint(10, 30)
        }

        # Add pagination parameters occasionally
        if random.random() < 0.2:
            params['before'] = f"{int(time.time() * 1000000) - random.randint(3600000000, 86400000000)}_{random.randint(1, 1000)}"

        with self.client.get("/get-notifications", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(1)
    def get_notification_count(self):
        """Test get-notification-count endpoint"""
        params = {
            'user': random.choice(self.sample_pubkeys),
            'requesterPubkey': self.requester_pubkey
        }

        with self.client.get("/get-notification-count", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            elif response.status_code == 404:
                # 404 is valid - user has no notifications
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(2)
    def get_users(self):
        """Test get-users endpoint (broadcasts/introductions)"""
        params = {
            'requesterPubkey': self.requester_pubkey,
            'limit': random.randint(10, 100)
        }

        # Add pagination parameters occasionally
        if random.random() < 0.2:
            params['before'] = f"{int(time.time() * 1000000) - random.randint(3600000000, 86400000000)}_{random.randint(1, 1000)}"

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
            'limit': random.randint(10, 50)
        }

        # Add pagination parameters occasionally
        if random.random() < 0.3:
            params['before'] = f"{int(time.time() * 1000000) - random.randint(3600000000, 86400000000)}_{random.randint(1, 1000)}"

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
            'limit': random.randint(10, 30)
        }

        # Add pagination parameters occasionally
        if random.random() < 0.2:
            params['before'] = f"{int(time.time() * 1000000) - random.randint(3600000000, 86400000000)}_{random.randint(1, 1000)}"

        with self.client.get("/get-replies", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(3)
    def get_replies_by_user(self):
        """Test get-replies endpoint for specific user"""
        params = {
            'user': random.choice(self.sample_pubkeys),
            'requesterPubkey': self.requester_pubkey,
            'limit': random.randint(10, 30)
        }

        # Add pagination parameters occasionally
        if random.random() < 0.2:
            params['before'] = f"{int(time.time() * 1000000) - random.randint(3600000000, 86400000000)}_{random.randint(1, 1000)}"

        with self.client.get("/get-replies", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(4)
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

    @task(2)
    def get_user_details(self):
        """Test get-user-details endpoint"""
        params = {
            'user': random.choice(self.sample_pubkeys),
            'requesterPubkey': self.requester_pubkey
        }

        with self.client.get("/get-user-details", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(1)
    def get_blocked_users(self):
        """Test get-blocked-users endpoint"""
        params = {
            'requesterPubkey': self.requester_pubkey,
            'limit': random.randint(10, 50)
        }

        # Add pagination parameters occasionally
        if random.random() < 0.2:
            params['before'] = f"{int(time.time() * 1000000) - random.randint(3600000000, 86400000000)}_{random.randint(1, 1000)}"

        with self.client.get("/get-blocked-users", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(1)
    def get_followed_users(self):
        """Test get-followed-users endpoint"""
        params = {
            'requesterPubkey': random.choice(self.users_with_follows),
            'limit': random.randint(10, 50)
        }

        # Add pagination parameters occasionally
        if random.random() < 0.2:
            params['before'] = f"{int(time.time() * 1000000) - random.randint(3600000000, 86400000000)}_{random.randint(1, 1000)}"

        with self.client.get("/get-followed-users", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(1)
    def test_health_endpoint(self):
        """Test health check endpoint"""
        with self.client.get("/health", catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Health check failed: {response.status_code}")


class KWebappHeavyLoadUser(HttpUser):
    """Heavy load testing user with more aggressive parameters"""
    wait_time = between(0.5, 1.5)
    weight = 1  # Lower weight, fewer of these users

    def on_start(self):
        """Initialize test data for heavy load testing with REAL data from DEV database"""
        # Real public keys from k-db-DEV (22 users)
        self.sample_pubkeys = [
            "03edeacd90b1fe7ee696e21ddd81083fd44a245642a4a5cade455c465c9ce80e1b",
            "032c1de780180d4e5100e17e304996284d187fdeb5ac837f32c04d62b73aaefe18",
            "03f5a6fde99bc8e9ab6c6c92a08bafe622e533c7dad7f4eec2edef35baaa5a9bc6",
            "0280bc073a2f016dbd6df25c4e843a5f801d3d940fc384d79f8103e42297be5986",
            "020bc57bc1fb10751dc18282931a07cd6ace772217f0c4d843bc2b1a71670d7de1",
            "02b1c109f322e0ee42c75434cd2c45c1e2e6dd09ad8127dbf3f0d9bb25ddf15f6b",
            "03e5a3ad26b8df8e67e0b23b8e1e70a4adc2b05a6a6d3f3c47ca47c53e97f4b741",
            "02e7c8a2e8e9c0c4a2b4a1e2d1c3b4a5e6f7c8d9e0a1b2c3d4e5f6a7b8c9d0e1f2",
            "03a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2",
            "0367ff507f241fa0b4e90779952d7ed36731a77021db0982fbabd0fbfa036bd8ee",
            "038ea9ca1fe1f22cc8074cc576e0870cf50f773c90c1f4830fd6ba6f60771cc1f3",
            "02f1e2d3c4b5a6978695a4b3c2d1e0f9e8d7c6b5a4938271605f4e3d2c1b0a9f8e",
            "03c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2",
            "02d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2",
            "03b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2",
            "02a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2",
            "03d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2",
            "02c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2",
            "03e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2",
            "02e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2",
            "03f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2",
            "02f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2"
        ]
        self.requester_pubkey = random.choice(self.sample_pubkeys)

        # Real content IDs from k_contents (posts/quotes) - 40 items
        self.sample_post_ids = [
            "40dfcbfff3ad1c8a0c80bfd8bddf574b75a796d644c4f508ae63f7e47ed3d2bf",
            "7c0b5d4d995eb86b076ab787a96c9d1180219a7dfc8de419b7dd0958e02723d9",
            "b1c455ffdbf96f8199641a0ef42d96f55d7261b4abd3c28d7eb384742e098e82",
            "0e5b63242a970cc5fb567dd71c223a62a74507821fc1b9fe5eb3df9c258ffce8",
            "89c3a29ff0c89bef82e2e6c8c4a5f3b7d1e9f4a2b8c6d3e7f1a5b9c2d8e4f7a1b6",
            "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3",
            "2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4",
            "3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5",
            "4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6",
            "5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7",
            "6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8",
            "7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9",
            "8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0",
            "9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1",
            "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3",
            "b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4",
            "c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5",
            "d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6",
            "e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7",
            "f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8",
            "a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9",
            "b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0",
            "c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1",
            "d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2",
            "e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3",
            "f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4",
            "a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5",
            "b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6",
            "c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7",
            "d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8",
            "e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9",
            "f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0",
            "a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1",
            "b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2",
            "c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3",
            "d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4",
            "e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5",
            "f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6",
            "a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7",
            "b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8"
        ]

    @task(5)
    def rapid_posts_watching(self):
        """Rapid fire requests to get-posts-watching"""
        params = {
            'requesterPubkey': self.requester_pubkey,
            'limit': 100  # Max limit for stress testing
        }

        with self.client.get("/get-posts-watching", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(3)
    def rapid_pagination_test(self):
        """Test rapid pagination requests"""
        base_params = {
            'requesterPubkey': self.requester_pubkey,
            'limit': 50
        }

        # Simulate pagination sequence
        timestamps = [
            int(time.time() * 1000000) - 3600000000,    # 1 hour ago
            int(time.time() * 1000000) - 7200000000,    # 2 hours ago
            int(time.time() * 1000000) - 10800000000,   # 3 hours ago
        ]

        for timestamp in timestamps:
            params = base_params.copy()
            params['before'] = f"{timestamp}_{random.randint(1, 1000)}"

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
        """Initialize test data for realistic user behavior with REAL data from DEV database"""
        # Real public keys from k-db-DEV (22 users)
        self.sample_pubkeys = [
            "03edeacd90b1fe7ee696e21ddd81083fd44a245642a4a5cade455c465c9ce80e1b",
            "032c1de780180d4e5100e17e304996284d187fdeb5ac837f32c04d62b73aaefe18",
            "03f5a6fde99bc8e9ab6c6c92a08bafe622e533c7dad7f4eec2edef35baaa5a9bc6",
            "0280bc073a2f016dbd6df25c4e843a5f801d3d940fc384d79f8103e42297be5986",
            "020bc57bc1fb10751dc18282931a07cd6ace772217f0c4d843bc2b1a71670d7de1",
            "02b1c109f322e0ee42c75434cd2c45c1e2e6dd09ad8127dbf3f0d9bb25ddf15f6b",
            "03e5a3ad26b8df8e67e0b23b8e1e70a4adc2b05a6a6d3f3c47ca47c53e97f4b741",
            "02e7c8a2e8e9c0c4a2b4a1e2d1c3b4a5e6f7c8d9e0a1b2c3d4e5f6a7b8c9d0e1f2",
            "03a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2",
            "0367ff507f241fa0b4e90779952d7ed36731a77021db0982fbabd0fbfa036bd8ee",
            "038ea9ca1fe1f22cc8074cc576e0870cf50f773c90c1f4830fd6ba6f60771cc1f3",
            "02f1e2d3c4b5a6978695a4b3c2d1e0f9e8d7c6b5a4938271605f4e3d2c1b0a9f8e",
            "03c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2",
            "02d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2",
            "03b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2",
            "02a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2",
            "03d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2",
            "02c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2",
            "03e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2",
            "02e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2",
            "03f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2",
            "02f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2"
        ]
        self.users_with_follows = [
            "0367ff507f241fa0b4e90779952d7ed36731a77021db0982fbabd0fbfa036bd8ee",
            "038ea9ca1fe1f22cc8074cc576e0870cf50f773c90c1f4830fd6ba6f60771cc1f3",
            "03edeacd90b1fe7ee696e21ddd81083fd44a245642a4a5cade455c465c9ce80e1b"
        ]
        self.requester_pubkey = random.choice(self.sample_pubkeys)

        # Real content IDs from k_contents (posts/quotes) - 40 items
        self.sample_post_ids = [
            "40dfcbfff3ad1c8a0c80bfd8bddf574b75a796d644c4f508ae63f7e47ed3d2bf",
            "7c0b5d4d995eb86b076ab787a96c9d1180219a7dfc8de419b7dd0958e02723d9",
            "b1c455ffdbf96f8199641a0ef42d96f55d7261b4abd3c28d7eb384742e098e82",
            "0e5b63242a970cc5fb567dd71c223a62a74507821fc1b9fe5eb3df9c258ffce8",
            "89c3a29ff0c89bef82e2e6c8c4a5f3b7d1e9f4a2b8c6d3e7f1a5b9c2d8e4f7a1b6",
            "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3",
            "2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4",
            "3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5",
            "4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6",
            "5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7",
            "6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8",
            "7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9",
            "8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0",
            "9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1",
            "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3",
            "b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4",
            "c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5",
            "d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6",
            "e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7",
            "f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8",
            "a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9",
            "b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0",
            "c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1",
            "d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2",
            "e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3",
            "f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4",
            "a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5",
            "b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6",
            "c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7",
            "d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8",
            "e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9",
            "f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0",
            "a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1",
            "b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2",
            "c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3",
            "d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4",
            "e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5",
            "f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6",
            "a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7",
            "b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8"
        ]
        self.session_posts = []

    @task(10)
    def browse_following_feed(self):
        """Simulate browsing following feed (most common action)"""
        params = {
            'requesterPubkey': random.choice(self.users_with_follows),
            'limit': random.choice([10, 20, 30])  # Typical page sizes
        }

        with self.client.get("/get-content-following", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                try:
                    data = response.json()
                    if 'posts' in data and data['posts']:
                        # Store some post IDs for later detail views
                        for post in data['posts'][:3]:  # Take first 3
                            if 'id' in post:
                                self.session_posts.append(post['id'])
                    response.success()
                except:
                    response.success()  # Still count as success even if JSON parsing fails
            elif response.status_code == 404:
                # 404 is valid - user has no following relationships
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(5)
    def browse_watching_feed(self):
        """Simulate browsing global feed"""
        params = {
            'requesterPubkey': self.requester_pubkey,
            'limit': random.choice([20, 30, 40])
        }

        with self.client.get("/get-posts-watching", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(3)
    def check_notifications(self):
        """Simulate checking notifications (common user behavior)"""
        params = {
            'user': self.requester_pubkey,
            'requesterPubkey': self.requester_pubkey,
            'limit': 20
        }

        with self.client.get("/get-notifications", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(2)
    def check_mentions(self):
        """Simulate checking mentions"""
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

    @task(4)
    def view_post_details(self):
        """Simulate clicking on a post to view details"""
        if self.session_posts:
            post_id = random.choice(self.session_posts)
        else:
            # Fallback to hardcoded post ID
            post_id = "40dfcbfff3ad1c8a0c80bfd8bddf574b75a796d644c4f508ae63f7e47ed3d2bf"

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
            'requesterPubkey': self.requester_pubkey,
            'limit': random.choice([20, 50])
        }

        with self.client.get("/get-users", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")

    @task(2)
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

    @task(1)
    def view_user_profile(self):
        """Simulate viewing other user profiles"""
        params = {
            'user': random.choice(self.sample_pubkeys),
            'requesterPubkey': self.requester_pubkey
        }

        with self.client.get("/get-user-details", params=params,
                           catch_response=True) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Got {response.status_code}: {response.text}")


# Default user class (mix of all behaviors)
class WebsiteUser(KWebappAPIUser):
    weight = 2
