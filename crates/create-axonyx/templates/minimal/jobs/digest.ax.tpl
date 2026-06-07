job PublishDailyDigest
  data posts = db.posts.all()
    where status = "published"
    order created_at desc
    limit 10
  send DigestEmail with posts
