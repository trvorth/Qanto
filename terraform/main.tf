terraform {
  required_version = ">= 1.5.0"
  
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 5.0"
    }
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.23"
    }
  }
  
  backend "gcs" {
    bucket = "qanto-terraform-state"
    prefix = "production/state"
  }
}

provider "google" {
  project = var.gcp_project_id
  region  = var.region
}

# VPC Network
resource "google_compute_network" "qanto_vpc" {
  name                    = "${var.cluster_name}-vpc"
  auto_create_subnetworks = false
  routing_mode           = "REGIONAL"
}

# Public subnet for load balancers
resource "google_compute_subnetwork" "public_subnet" {
  name          = "${var.cluster_name}-public"
  network       = google_compute_network.qanto_vpc.id
  region        = var.region
  ip_cidr_range = "10.0.1.0/24"
  
  secondary_ip_range {
    range_name    = "pods"
    ip_cidr_range = "10.1.0.0/16"
  }
  
  secondary_ip_range {
    range_name    = "services"
    ip_cidr_range = "10.2.0.0/16"
  }
}

# Private subnet for nodes
resource "google_compute_subnetwork" "private_subnet" {
  name                     = "${var.cluster_name}-private"
  network                  = google_compute_network.qanto_vpc.id
  region                   = var.region
  ip_cidr_range           = "10.0.2.0/24"
  private_ip_google_access = true
}

# Cloud NAT for private nodes
resource "google_compute_router" "nat_router" {
  name    = "${var.cluster_name}-nat-router"
  network = google_compute_network.qanto_vpc.id
  region  = var.region
}

resource "google_compute_router_nat" "nat_gateway" {
  name                               = "${var.cluster_name}-nat"
  router                            = google_compute_router.nat_router.name
  region                            = var.region
  nat_ip_allocate_option            = "AUTO_ONLY"
  source_subnetwork_ip_ranges_to_nat = "ALL_SUBNETWORKS_ALL_IP_RANGES"
}

# Static IP for boot node
resource "google_compute_address" "boot_node_ip" {
  name   = "${var.cluster_name}-boot-node"
  region = var.region
}

# Static IP for website ingress
resource "google_compute_global_address" "website_ip" {
  name = "${var.cluster_name}-website"
}

# GKE Cluster
resource "google_container_cluster" "qanto_cluster" {
  name     = var.cluster_name
  location = var.zone
  
  # Use VPC
  network    = google_compute_network.qanto_vpc.id
  subnetwork = google_compute_subnetwork.public_subnet.id
  
  # Remove default node pool
  remove_default_node_pool = true
  initial_node_count       = 1
  
  # Cluster configuration
  cluster_autoscaling {
    enabled = true
    resource_limits {
      resource_type = "cpu"
      minimum       = 4
      maximum       = 100
    }
    resource_limits {
      resource_type = "memory"
      minimum       = 16
      maximum       = 400
    }
  }
  
  # Security settings
  private_cluster_config {
    enable_private_nodes    = false  # Public IPs for initial setup
    enable_private_endpoint = false
    master_ipv4_cidr_block = "172.16.0.0/28"
  }
  
  master_auth {
    client_certificate_config {
      issue_client_certificate = false
    }
  }
  
  # IP allocation
  ip_allocation_policy {
    cluster_secondary_range_name  = "pods"
    services_secondary_range_name = "services"
  }
  
  # Workload Identity
  workload_identity_config {
    workload_pool = "${var.gcp_project_id}.svc.id.goog"
  }
  
  # Add-ons
  addons_config {
    http_load_balancing {
      disabled = false
    }
    horizontal_pod_autoscaling {
      disabled = false
    }
    network_policy_config {
      disabled = false
    }
  }
  
  # Maintenance window
  maintenance_policy {
    daily_maintenance_window {
      start_time = "03:00"
    }
  }
}

# Node pool for blockchain workloads
resource "google_container_node_pool" "blockchain_pool" {
  name       = "blockchain-pool"
  cluster    = google_container_cluster.qanto_cluster.id
  location   = var.zone
  node_count = var.blockchain_node_count
  
  autoscaling {
    min_node_count = var.blockchain_min_nodes
    max_node_count = var.blockchain_max_nodes
  }
  
  management {
    auto_repair  = true
    auto_upgrade = true
  }
  
  node_config {
    preemptible  = false
    machine_type = var.blockchain_machine_type
    
    disk_size_gb = 100
    disk_type    = "pd-ssd"
    
    metadata = {
      disable-legacy-endpoints = "true"
    }
    
    oauth_scopes = [
      "https://www.googleapis.com/auth/cloud-platform"
    ]
    
    labels = {
      workload = "blockchain"
    }
    
    tags = ["blockchain-node"]
    
    workload_metadata_config {
      mode = "GKE_METADATA"
    }
  }
}

# Node pool for web workloads
resource "google_container_node_pool" "web_pool" {
  name       = "web-pool"
  cluster    = google_container_cluster.qanto_cluster.id
  location   = var.zone
  node_count = var.web_node_count
  
  autoscaling {
    min_node_count = var.web_min_nodes
    max_node_count = var.web_max_nodes
  }
  
  management {
    auto_repair  = true
    auto_upgrade = true
  }
  
  node_config {
    preemptible  = var.web_use_preemptible
    machine_type = var.web_machine_type
    
    disk_size_gb = 50
    disk_type    = "pd-standard"
    
    metadata = {
      disable-legacy-endpoints = "true"
    }
    
    oauth_scopes = [
      "https://www.googleapis.com/auth/cloud-platform"
    ]
    
    labels = {
      workload = "web"
    }
    
    tags = ["web-node"]
    
    workload_metadata_config {
      mode = "GKE_METADATA"
    }
  }
}

# Firewall rules
resource "google_compute_firewall" "qanto_p2p" {
  name    = "${var.cluster_name}-p2p"
  network = google_compute_network.qanto_vpc.name
  
  allow {
    protocol = "tcp"
    ports    = ["8333"]
  }
  
  source_ranges = ["0.0.0.0/0"]
  target_tags   = ["blockchain-node"]
}

resource "google_compute_firewall" "qanto_api" {
  name    = "${var.cluster_name}-api"
  network = google_compute_network.qanto_vpc.name
  
  allow {
    protocol = "tcp"
    ports    = ["8080", "8081"]
  }
  
  source_ranges = ["0.0.0.0/0"]
  target_tags   = ["blockchain-node"]
}
