This document proposes a new feature for the `dns-server` to implement.

Currently, there are 3 ways for a DNS query to be blocked:

- By the upstream resolver, if using something like CloudFlare Family or Mullvad
- By the HaGeZi blocklists, which are fetched on startup
- By the explicit blocklist, which the user can add domains to

These are only updated periodically, so I'd like to include something more
autonomous here. The idea is to store all the domains that have been previously
requested and evaluate new ones when they are requested for the first time.

For example, if the server receives a query for `opentracker.app` then it will:

- Make an HTTP request for the root document at `opentracker.app`
- If one exists, it will extract all the text from it (removing any styling,
  scripts, images, etc.)
- After this, it will send it to a decision model using OpenRouter (and the
  `openrouter-rs` crate)

The questions and thresholds for the model will be stored in the database so the
user can configure what types of domains they'd like to block. If the response
for any question is over the provided threshold, the domain should be blocked.

Once a decision on a domain has been made, the server should store this
information so it doesn't need to be performed again. Entries should contain
when the decision was made, what domain it was on and whether it was blocked or
allowed.

I'm imagining we will need a couple of new traits for this. We'll want a
`DomainClassifier` struct which requires implementations of the `DomainFetcher`
trait and the `DomainDecider` trait. The production implementation of the former
will use `reqwest` to grab the domain content and trim it down, the
implementation of the latter will be responsibly for calling OpenRouter and
handling the response. This should all go in its own separate module in a
subdirectory of `src`.
