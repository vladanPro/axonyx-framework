job PublishDailyDigest
  data posts = Query.PublishedPosts()
  send DigestEmail with posts
