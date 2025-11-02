A user can activate a specific utility called K-database-cleaner which allows to run a local indexer where the database is lighter and cleaned from unwanted contents: depending on the settings, some data are being automatically removed from database.

**General behavior:**
All K-transaction-processor operations keep working as in the current implementation.

In case the user wants to keep the database lighter and cleaner, he activate the new utility K-database-cleaner which runs every X seconds/minutes (depending on user preferences) a purge function and a series of database queries to:
- remove all contents (posts, quotes, replies, votes) from blocked users, including related data from k_mentions table;
- remove all posts and quotes from non-followed users, older than X hours/days (depending on users setting), including related data from k_mentions table;
- remove all replies which aren't related to the user's contents or to followed users' contents;
- remove all votes which are related those removed contents.

**Implementation**:
The implementation of the purge fuction in K-database-cleaner have be follow this sequence:
1. First of all, a query to remove all records where sender_pubkey is not the one specified in the "user" parameter, from the k_blocks and k_follows tables; report on log how many contents have been deleted in each table.
2. After the previous query have been successfully executed, run a query to remove all contents (posts, quotes, replies, votes) where sender_pubkey is a blocked user (reading the blocked users from k_blocks table) on k_contents and k_votes table, including related data on k_mentions table; report on log how many contents have been deleted in each table;
3. After the previous query have been successfully executed, run a query which removes from k_contents all posts and quotes from non-followed users (where sender_pubkey in k_contents is not followed by the user in k_follows table), older than X hours (depending on users parameter called data-retention), including related data from k_mentions table; report on log how many contents have been deleted in each table.
4. After the previous query have been successfully executed, run a query which removes from k_contents all replies which refer to referenced_content_id which aren't present in the database, including related data from k_mentions table; report on log how many contents have been deleted in each table.
5. After the previous query have been successfully executed, run a query which removes from k_votes all votes which refer to post_id which aren't present in the database, including related data from k_mentions table; report on log how many contents have been deleted in each table.

K-database-cleaner do not remove anything from the following table:
- k_broadcasts

**CLI parameters**:
These are the parameters which can be applied to the K-database-cleaner:
- "user" for long or "u" for short: indicates the public key of the user to whom this indexer is dedicated to;
- "purge-interval" for long or "pt" for short: set the interval (in seconds) between one purge operation and the following one; if not indicated, default value is 600;
- "data-retention" for long or "dr" for short: set the amount of time to retain the data and purge the older data, as mentioned in point 3 of the implementation above.

