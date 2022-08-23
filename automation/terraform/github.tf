terraform {
  required_providers {
    github = {
      source  = "integrations/github"
      version = "~> 4.0"
    }
  }
}

resource "github_repository" "twinkle" {
  name        = "twinkle"
  description = ""

  visibility = "public"
  auto_init = true
}

resource "github_branch" "master" {
  repository = github_repository.twinkle.name
  branch     = "master"
}

resource "github_branch_default" "default"{
  repository = github_repository.twinkle.name
  branch     = github_branch.master.branch
}