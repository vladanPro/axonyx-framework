job PublishDailyDigest
  data posts = Db.Stream("posts")
    where status = "published"
    order created_at desc
    limit 10
  send DigestEmail with posts
