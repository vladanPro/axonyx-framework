job PublishDailyDigest
  data posts = db.posts.all()
    where status = "published"
    order created_at desc
    limit 5
  send DigestEmail with posts
